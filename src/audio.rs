use crate::state::AppState;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig};
use num_complex::Complex;
use rustfft::FftPlanner;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use webrtc_vad::{SampleRate as VADSampleRate, Vad, VadMode as VADMode};

const DEFAULT_SAMPLE_RATE: u32 = 24000;
const HANGOVER_STRICT_MS: u128 = 480;
const HANGOVER_LENIENT_MS: u128 = 700;
const PREROLL_STRICT_MS: f64 = 220.0;
const PREROLL_LENIENT_MS: f64 = 300.0;
const MIN_TURN_STRICT_MS: f64 = 35.0;
const MIN_TURN_LENIENT_MS: f64 = 10.0;
const STOP_SILENCE_STRICT_MS: f64 = 80.0;
const STOP_SILENCE_LENIENT_MS: f64 = 60.0;
const POST_ROLL_STRICT_MS: f64 = 80.0;
const POST_ROLL_LENIENT_MS: f64 = 80.0;
const VAD_SAMPLE_RATE: u32 = 16000;
const VAD_FRAME_MS: usize = 20;
const VAD_START_TRIGGER_FRAMES: usize = 2;
const FFT_SIZE: usize = 256;
const BAR_COUNT: usize = 50;

pub struct AudioCapture {
    _stream: cpal::Stream,
    _processor: Option<std::thread::JoinHandle<()>>,
}

impl AudioCapture {
    pub fn start(
        device_name: Option<&str>,
        audio_tx: mpsc::Sender<Vec<u8>>,
        state: Arc<AppState>,
        target_rate: u32,
    ) -> Result<Self, String> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| format!("Failed to list devices: {}", e))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| format!("Device '{}' not found", name))?
        } else {
            host.default_input_device()
                .ok_or("No default input device")?
        };

        let device_name = device.name().unwrap_or_else(|_| "unknown".into());
        println!("[audio] using device: {}", device_name);

        // Try target sample rate mono, fall back to 48kHz
        let (config, decimate) = match try_config(&device, target_rate) {
            Some(cfg) => (cfg, 1),
            None => match try_config(&device, 48000) {
                Some(cfg) => {
                    let d = (cfg.sample_rate.0 / target_rate.max(1)).max(1);
                    println!(
                        "[audio] {}Hz unavailable, using {}Hz with {}:1 decimation",
                        target_rate,
                        cfg.sample_rate.0,
                        d
                    );
                    (cfg, d)
                }
                None => {
                    // Last resort: use default config
                    let default = device
                        .default_input_config()
                        .map_err(|e| format!("No input config: {}", e))?;
                    println!(
                        "[audio] using default config: {}Hz {}ch",
                        default.sample_rate().0,
                        default.channels()
                    );
                    let rate = default.sample_rate().0;
                    let d = (rate / target_rate.max(1)).max(1);
                    (
                        StreamConfig {
                            channels: 1,
                            sample_rate: default.sample_rate(),
                            buffer_size: cpal::BufferSize::Default,
                        },
                        d,
                    )
                }
            },
        };

        let effective_rate = config.sample_rate.0 / decimate;
        println!(
            "[audio] stream config: {}Hz, {}ch, decimate={}, effective={}Hz",
            config.sample_rate.0, config.channels, decimate, effective_rate
        );

        // Channel from cpal callback to processing thread
        let (raw_tx, raw_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(128);

        let channels = config.channels as usize;
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Downmix to mono if stereo
                    let mono: Vec<f32> = if channels > 1 {
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };
                    // Decimate if needed
                    let samples: Vec<f32> = if decimate > 1 {
                        mono.iter().step_by(decimate as usize).copied().collect()
                    } else {
                        mono
                    };
                    let _ = raw_tx.try_send(samples);
                },
                |err| {
                    eprintln!("[audio] stream error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start stream: {}", e))?;

        let processor = std::thread::spawn(move || {
            let target = if target_rate == 0 {
                DEFAULT_SAMPLE_RATE
            } else {
                target_rate
            };
            process_audio(raw_rx, audio_tx, state, effective_rate, target);
        });

        Ok(Self {
            _stream: stream,
            _processor: Some(processor),
        })
    }
}

fn try_config(device: &cpal::Device, rate: u32) -> Option<StreamConfig> {
    let config = StreamConfig {
        channels: 1,
        sample_rate: SampleRate(rate),
        buffer_size: cpal::BufferSize::Default,
    };
    // Check if device supports this config by trying to build (we can't easily check
    // supported configs without iterating). Just return the config optimistically.
    // If it fails, the caller will try fallback.
    let supported = device.supported_input_configs().ok()?;
    for range in supported {
        if range.channels() == 1
            && range.min_sample_rate().0 <= rate
            && range.max_sample_rate().0 >= rate
        {
            return Some(config);
        }
    }
    // Also check stereo configs (we'll downmix)
    let supported = device.supported_input_configs().ok()?;
    for range in supported {
        if range.min_sample_rate().0 <= rate && range.max_sample_rate().0 >= rate {
            return Some(StreamConfig {
                channels: range.channels(),
                sample_rate: SampleRate(rate),
                buffer_size: cpal::BufferSize::Default,
            });
        }
    }
    None
}

fn process_audio(
    raw_rx: std::sync::mpsc::Receiver<Vec<f32>>,
    audio_tx: mpsc::Sender<Vec<u8>>,
    state: Arc<AppState>,
    input_rate: u32,
    target_rate: u32,
) {
    let mut last_voice_ts = Instant::now() - std::time::Duration::from_secs(10);
    let mut is_sending = false;
    let mut pending_stop = false;
    let mut post_roll_remaining_ms = 0.0f64;
    let mut voiced_ms = 0.0f64;
    let mut silence_ms = 0.0f64;
    let mut preroll: VecDeque<Vec<u8>> = VecDeque::new();
    let mut preroll_ms = 0.0;
    let mut resampler = ResamplerState::default();
    let mut vad_resampler = ResamplerState::default();
    let mut vad = Vad::new_with_rate_and_mode(VADSampleRate::Rate16kHz, VADMode::Aggressive);
    let mut vad_frame_buf: Vec<i16> = Vec::with_capacity((VAD_SAMPLE_RATE as usize / 1000) * 60);
    let vad_frame_samples: usize = (VAD_SAMPLE_RATE as usize * VAD_FRAME_MS) / 1000;
    let mut speech_run_frames: usize = 0;

    // FFT setup — accumulate samples in a ring buffer since chunks may be < FFT_SIZE
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut fft_ring = Vec::with_capacity(FFT_SIZE * 2);
    let mut fft_buffer = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    let mut fft_smoothed = [0.0f32; BAR_COUNT];

    while let Ok(samples) = raw_rx.recv() {
        // Resample to target rate if needed, then convert to 16-bit PCM.
        let send_samples = if input_rate == target_rate {
            samples.clone()
        } else {
            resample_linear(&samples, input_rate, target_rate, &mut resampler)
        };
        let pcm: Vec<u8> = send_samples
            .iter()
            .flat_map(|&s| {
                let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                clamped.to_le_bytes()
            })
            .collect();

        // Peak amplitude for logs/debug (VAD classification uses WebRTC VAD below).
        let peak = send_samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let mode = state.vad_mode.load(std::sync::atomic::Ordering::SeqCst);
        let (
            hangover_ms,
            preroll_target,
            min_turn_ms,
            stop_silence_ms,
            post_roll_ms,
            vad_aggressiveness,
            vad_label,
        ) = match mode {
            2 => (
                HANGOVER_LENIENT_MS,
                PREROLL_LENIENT_MS,
                0.0,
                0.0,
                0.0,
                VADMode::LowBitrate,
                "off",
            ), // legacy off: always send
            1 => (
                HANGOVER_LENIENT_MS,
                PREROLL_LENIENT_MS,
                MIN_TURN_LENIENT_MS,
                STOP_SILENCE_LENIENT_MS,
                POST_ROLL_LENIENT_MS,
                VADMode::Quality,
                "lenient",
            ),
            _ => (
                HANGOVER_STRICT_MS,
                PREROLL_STRICT_MS,
                MIN_TURN_STRICT_MS,
                STOP_SILENCE_STRICT_MS,
                POST_ROLL_STRICT_MS,
                VADMode::Aggressive,
                "strict",
            ),
        };
        vad.set_mode(vad_aggressiveness);

        // Feed WebRTC VAD from a 16k side-stream using fixed 20 ms frames.
        let vad_samples = if input_rate == VAD_SAMPLE_RATE {
            samples.clone()
        } else {
            resample_linear(&samples, input_rate, VAD_SAMPLE_RATE, &mut vad_resampler)
        };
        for &s in &vad_samples {
            let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
            vad_frame_buf.push(clamped);
        }

        let mut chunk_any_speech = false;
        while vad_frame_buf.len() >= vad_frame_samples {
            let is_speech = vad
                .is_voice_segment(&vad_frame_buf[..vad_frame_samples])
                .unwrap_or(false);
            vad_frame_buf.drain(..vad_frame_samples);
            if is_speech {
                chunk_any_speech = true;
                speech_run_frames = speech_run_frames.saturating_add(1);
            } else {
                speech_run_frames = 0;
            }
        }

        // Start requires a small speech streak to avoid noise spikes.
        let has_voice = if mode == 2 {
            true
        } else if is_sending {
            chunk_any_speech
        } else {
            speech_run_frames >= VAD_START_TRIGGER_FRAMES
        };
        let now = Instant::now();
        if has_voice {
            last_voice_ts = now;
            silence_ms = 0.0;
            if pending_stop {
                pending_stop = false;
                post_roll_remaining_ms = 0.0;
            }
        }
        let in_hangover = now.duration_since(last_voice_ts).as_millis() <= hangover_ms;

        let chunk_ms = (send_samples.len() as f64 / target_rate as f64) * 1000.0;
        if has_voice {
            voiced_ms += chunk_ms;
        } else {
            silence_ms += chunk_ms;
        }

        // Accumulate samples for FFT
        fft_ring.extend_from_slice(&samples);
        // Keep only the latest window (avoid unbounded growth)
        if fft_ring.len() > FFT_SIZE * 2 {
            let drain = fft_ring.len() - FFT_SIZE * 2;
            fft_ring.drain(..drain);
        }

        // Compute FFT when we have enough samples
        if fft_ring.len() >= FFT_SIZE {
            let start = fft_ring.len() - FFT_SIZE;
            for i in 0..FFT_SIZE {
                let window = 0.5
                    * (1.0
                        - (2.0 * std::f32::consts::PI * i as f32
                            / (FFT_SIZE as f32 - 1.0))
                            .cos());
                fft_buffer[i] = Complex::new(fft_ring[start + i] * window, 0.0);
            }
            fft.process(&mut fft_buffer);

            // Extract BAR_COUNT bars from lower frequency bins (skip DC at 0)
            let max_bin = FFT_SIZE / 2;
            for i in 0..BAR_COUNT {
                let idx = 1 + ((i as f32 / BAR_COUNT as f32) * (max_bin as f32 - 1.0)) as usize;
                let idx = idx.min(max_bin - 1);
                let mag = fft_buffer[idx].norm();
                // Scale: typical speech FFT magnitudes are small; normalize gently.
                let normalized = (mag * 0.4).min(1.0);
                fft_smoothed[i] = fft_smoothed[i] * 0.6 + normalized * 0.4;
            }
            if let Ok(mut data) = state.fft_data.lock() {
                *data = fft_smoothed;
            }
        }

        // Preroll buffer
        preroll.push_back(pcm.clone());
        preroll_ms += chunk_ms;
        while preroll_ms > preroll_target {
            if let Some(dropped) = preroll.front() {
                let dropped_ms = dropped.len() as f64 / 2.0 / target_rate as f64 * 1000.0;
                preroll_ms -= dropped_ms;
                let _ = preroll.pop_front();
            } else {
                break;
            }
        }

        if pending_stop {
            let _ = audio_tx.try_send(pcm.clone());
            post_roll_remaining_ms -= chunk_ms;
            if post_roll_remaining_ms <= 0.0 {
                println!(
                    "[audio] VAD commit: post_roll_ms={:.1} mode={}",
                    post_roll_ms, vad_label
                );
                send_commit_signal(&audio_tx, "[audio] commit post-roll");
                pending_stop = false;
                is_sending = false;
                voiced_ms = 0.0;
                silence_ms = 0.0;
            } else {
                is_sending = true;
            }
            continue;
        }

        if !has_voice && silence_ms >= stop_silence_ms && !in_hangover {
            let suppressed_ms = chunk_ms.max(0.0) as u64;
            if suppressed_ms > 0 {
                if let Ok(mut usage) = state.usage.lock() {
                    usage.ms_suppressed = usage.ms_suppressed.saturating_add(suppressed_ms);
                    usage.last_update_ms = now_ms();
                }
                let mut provider_key: Option<String> = None;
                if let Ok(mut session) = state.session_usage.lock() {
                    if session.started_ms != 0 {
                        session.ms_suppressed = session.ms_suppressed.saturating_add(suppressed_ms);
                        session.updated_ms = now_ms();
                        if !session.provider.is_empty() {
                            provider_key = Some(session.provider.clone());
                        }
                    }
                }
                if let Some(provider) = provider_key {
                    if let Ok(mut pt) = state.provider_totals.lock() {
                        let entry = pt.entry(provider).or_default();
                        entry.ms_suppressed = entry.ms_suppressed.saturating_add(suppressed_ms);
                    }
                }
            }
            if is_sending {
                println!(
                    "[audio] VAD stop: peak={:.5} mode={} hangover_ms={} stop_silence_ms={:.1} preroll_ms={:.1}",
                    peak, vad_label, hangover_ms, stop_silence_ms, preroll_ms
                );
                if voiced_ms >= min_turn_ms {
                    pending_stop = post_roll_ms > 0.0;
                    post_roll_remaining_ms = post_roll_ms;
                    if !pending_stop {
                        send_commit_signal(&audio_tx, "[audio] commit immediate");
                        is_sending = false;
                        voiced_ms = 0.0;
                        silence_ms = 0.0;
                    }
                } else {
                    println!(
                        "[audio] dropping micro-turn: voiced_ms={:.1} < min_turn_ms={:.1}",
                        voiced_ms, min_turn_ms
                    );
                    is_sending = false;
                    voiced_ms = 0.0;
                    silence_ms = 0.0;
                }
            }
            continue;
        }

        if has_voice && !is_sending {
            println!(
                "[audio] VAD start: peak={:.5} mode={} preroll_ms={:.1}",
                peak, vad_label, preroll_ms
            );
            is_sending = true;
            for buf in &preroll {
                let _ = audio_tx.try_send(buf.clone());
            }
            preroll.clear();
            preroll_ms = 0.0;
        }

        if is_sending {
            let _ = audio_tx.try_send(pcm);
        }
    }

    // Clear FFT when stream stops
    if let Ok(mut data) = state.fft_data.lock() {
        *data = [0.0; BAR_COUNT];
    }
    println!("[audio] processing thread stopped");
}

fn send_commit_signal(audio_tx: &mpsc::Sender<Vec<u8>>, context: &str) {
    for attempt in 1..=25 {
        match audio_tx.try_send(Vec::new()) {
            Ok(()) => return,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                std::thread::sleep(std::time::Duration::from_millis(4));
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                eprintln!("[audio] {} failed: channel closed", context);
                return;
            }
        }
        if attempt == 25 {
            eprintln!(
                "[audio] {} dropped after retries: audio channel remained full",
                context
            );
        }
    }
}

#[derive(Default)]
struct ResamplerState {
    t: f64,
    last_sample: f32,
    has_last: bool,
}

fn resample_linear(
    samples: &[f32],
    input_rate: u32,
    target_rate: u32,
    state: &mut ResamplerState,
) -> Vec<f32> {
    if samples.is_empty() || input_rate == target_rate {
        return samples.to_vec();
    }
    let step = input_rate as f64 / target_rate as f64;
    let mut out = Vec::with_capacity(((samples.len() as f64 / step) + 2.0) as usize);

    let mut buf = Vec::with_capacity(samples.len() + 1);
    if state.has_last {
        buf.push(state.last_sample);
    }
    buf.extend_from_slice(samples);

    let mut i: usize = 0;
    let mut t = state.t;
    while i + 1 < buf.len() {
        let s0 = buf[i];
        let s1 = buf[i + 1];
        let v = s0 + (s1 - s0) * t as f32;
        out.push(v);
        t += step;
        while t >= 1.0 {
            t -= 1.0;
            i += 1;
            if i + 1 >= buf.len() {
                break;
            }
        }
        if i + 1 >= buf.len() {
            break;
        }
    }

    state.t = t;
    if let Some(last) = buf.last() {
        state.last_sample = *last;
        state.has_last = true;
    }
    out
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// List available input devices (name strings).
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let devices = match host.input_devices() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    devices
        .filter_map(|d| d.name().ok())
        .collect()
}
