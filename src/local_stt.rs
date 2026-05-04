use crate::state::{AppEvent, AppState};
use crate::typing;
use crate::whisper_runtime::{pcm16le_to_f32_mono, WhisperRuntime};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender as EventSender;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

pub const OFFLINE_SAMPLE_RATE: u32 = 16_000;

const WHISPER_PROVIDER_ID: &str = "whisper";
const WHISPER_MODEL_LABEL: &str = "base.en-q5_1";

fn whisper_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(4, 8)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn emit_status(tx: &EventSender<AppEvent>, status: &str, message: &str) {
    let _ = tx.send(AppEvent::StatusUpdate {
        status: status.into(),
        message: message.into(),
    });
}

fn emit_transcript(tx: &EventSender<AppEvent>, text: &str, is_final: bool) {
    if is_final {
        let _ = tx.send(AppEvent::TranscriptFinal(text.into()));
    } else {
        let _ = tx.send(AppEvent::TranscriptDelta(text.into()));
    }
}

fn short_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn sanitize_transcript_text(text: &str) -> String {
    let mut cleaned = text
        .replace("[BLANK_AUDIO]", " ")
        .replace("[ Silence ]", " ")
        .replace("[Silence]", " ")
        .replace("[SILENCE]", " ")
        .replace("[MUSIC]", " ")
        .replace("[ Music ]", " ");
    cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned.trim().to_string()
}

fn normalize_alpha_words(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphabetic() || ch.is_ascii_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn should_suppress_whisper_short_hallucination(text: &str, _utterance_ms: u64) -> bool {
    normalize_alpha_words(text) == "you"
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn whisper_model_path() -> PathBuf {
    repo_root()
        .join(".models")
        .join("whispercpp")
        .join(format!("ggml-{}.bin", WHISPER_MODEL_LABEL))
}

async fn load_whisper_runtime() -> Result<Arc<WhisperRuntime>, String> {
    let model_path = whisper_model_path();
    let threads = whisper_thread_count() as i32;
    app_log!(
        "[whisper] runtime starting: threads={} model={} path={}",
        threads,
        WHISPER_MODEL_LABEL,
        model_path.display()
    );
    let runtime = tokio::task::spawn_blocking(move || WhisperRuntime::new(&model_path, threads))
        .await
        .map_err(|e| format!("Whisper runtime init join error: {}", e))??;
    let runtime = Arc::new(runtime);
    app_log!(
        "[whisper] runtime ready: threads={} model={}",
        threads,
        WHISPER_MODEL_LABEL
    );
    Ok(runtime)
}

async fn get_or_init_whisper_runtime(state: &Arc<AppState>) -> Result<Arc<WhisperRuntime>, String> {
    if let Ok(guard) = state.whisper_runtime.lock() {
        if let Some(runtime) = guard.as_ref() {
            return Ok(runtime.clone());
        }
    }

    let runtime = load_whisper_runtime().await?;

    let mut guard = state
        .whisper_runtime
        .lock()
        .map_err(|_| "Whisper runtime cache lock poisoned".to_string())?;
    if let Some(existing) = guard.as_ref() {
        Ok(existing.clone())
    } else {
        *guard = Some(runtime.clone());
        Ok(runtime)
    }
}

pub async fn preload_whisper_runtime(state: Arc<AppState>) -> Result<(), String> {
    let _ = get_or_init_whisper_runtime(&state).await?;
    Ok(())
}

pub fn unload_whisper_runtime(state: &Arc<AppState>, reason: &str) -> bool {
    let mut unloaded = false;
    if let Ok(mut guard) = state.whisper_runtime.lock() {
        if guard.is_some() {
            *guard = None;
            unloaded = true;
        }
    }
    if unloaded {
        app_log!("[whisper] runtime unloaded: reason={}", reason);
    }
    unloaded
}

pub fn check_offline_ready() -> Result<(), String> {
    let model = whisper_model_path();
    if !model.exists() {
        return Err(format!(
            "Whisper model not found at {}",
            model.display()
        ));
    }
    Ok(())
}

fn update_audio_usage(state: &Arc<AppState>, provider: &str, model: &str, chunk_bytes: usize) {
    let chunk_bytes = chunk_bytes as u64;
    let chunk_ms =
        ((chunk_bytes as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
    let now = now_ms();

    if let Ok(mut usage) = state.usage.lock() {
        usage.provider = provider.to_string();
        usage.model = model.to_string();
        usage.bytes_sent = usage.bytes_sent.saturating_add(chunk_bytes);
        usage.ms_sent = usage.ms_sent.saturating_add(chunk_ms);
        usage.last_update_ms = now;
    }
    if let Ok(mut session) = state.session_usage.lock() {
        if session.started_ms != 0 {
            session.provider = provider.to_string();
            session.model = model.to_string();
            session.bytes_sent = session.bytes_sent.saturating_add(chunk_bytes);
            session.ms_sent = session.ms_sent.saturating_add(chunk_ms);
            session.updated_ms = now;
        }
    }
    if let Ok(mut pt) = state.provider_totals.lock() {
        let entry = pt.entry(provider.to_string()).or_default();
        entry.bytes_sent = entry.bytes_sent.saturating_add(chunk_bytes);
        entry.ms_sent = entry.ms_sent.saturating_add(chunk_ms);
    }
}

fn update_commit_usage(state: &Arc<AppState>) {
    let now = now_ms();
    if let Ok(mut usage) = state.usage.lock() {
        usage.commits = usage.commits.saturating_add(1);
        usage.last_update_ms = now;
    }
    if let Ok(mut session) = state.session_usage.lock() {
        if session.started_ms != 0 {
            session.commits = session.commits.saturating_add(1);
            session.updated_ms = now;
        }
    }
}

fn handle_final_transcript(event_tx: &EventSender<AppEvent>, state: &Arc<AppState>, transcript: String) {
    let text = sanitize_transcript_text(&transcript);
    if text.is_empty() {
        return;
    }

    emit_transcript(event_tx, &text, true);

    if let Ok(mut usage) = state.usage.lock() {
        usage.finals = usage.finals.saturating_add(1);
    }
    if let Ok(mut session) = state.session_usage.lock() {
        if session.started_ms != 0 {
            session.finals = session.finals.saturating_add(1);
        }
    }
    if let Ok(mut pt) = state.provider_totals.lock() {
        let entry = pt.entry(WHISPER_PROVIDER_ID.to_string()).or_default();
        entry.finals = entry.finals.saturating_add(1);
    }
    if let Ok(mut last) = state.last_transcript.lock() {
        *last = text.clone();
    }

    let chrome = state
        .chrome_path
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let paint = state
        .paint_path
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let urls = state
        .url_commands
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let aliases = state
        .alias_commands
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let apps = state
        .app_shortcuts
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();

    tokio::task::spawn_blocking(move || {
        typing::process_transcript(&text, &chrome, &paint, &urls, &aliases, &apps);
    });
}

pub async fn run_session(
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
    inactivity_timeout_secs: u64,
) {
    if let Err(e) =
        run_whisper_session(event_tx.clone(), state.clone(), &mut audio_rx, inactivity_timeout_secs).await
    {
        emit_status(&event_tx, "error", &e);
    }
    emit_status(&event_tx, "idle", "Ready");
}

async fn whisper_transcribe_utterance(
    runtime: &Arc<WhisperRuntime>,
    pcm: Vec<u8>,
) -> Result<String, String> {
    let runtime = runtime.clone();
    tokio::task::spawn_blocking(move || {
        let samples = pcm16le_to_f32_mono(&pcm);
        runtime.transcribe(&samples)
    })
    .await
    .map_err(|e| format!("Whisper task join error: {}", e))?
}

async fn run_whisper_session(
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    audio_rx: &mut mpsc::Receiver<Vec<u8>>,
    inactivity_timeout_secs: u64,
) -> Result<(), String> {
    let runtime = get_or_init_whisper_runtime(&state).await?;
    emit_status(&event_tx, "live", "Listening (offline)");

    let provider = WHISPER_PROVIDER_ID;
    let model = WHISPER_MODEL_LABEL;
    let inactivity_timeout_ms = if inactivity_timeout_secs == 0 {
        None
    } else {
        Some(inactivity_timeout_secs.clamp(5, 300).saturating_mul(1000))
    };
    let mut last_activity_ms = now_ms();
    let mut inactivity_check = tokio::time::interval(Duration::from_secs(1));
    inactivity_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    inactivity_check.tick().await;
    let mut utterance_pcm = Vec::new();
    let mut utterance_seq: u64 = 0;

    loop {
        tokio::select! {
            audio = audio_rx.recv() => {
                let chunk = match audio {
                    Some(c) => c,
                    None => break,
                };

                if chunk.is_empty() {
                    if utterance_pcm.is_empty() {
                        continue;
                    }
                    utterance_seq = utterance_seq.saturating_add(1);
                    let utterance_ms =
                        ((utterance_pcm.len() as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
                    app_log!(
                        "[whisper] tx utterance: seq={} bytes={} ms={}",
                        utterance_seq,
                        utterance_pcm.len(),
                        utterance_ms
                    );
                    update_commit_usage(&state);
                    let started = now_ms();
                    let pcm = std::mem::take(&mut utterance_pcm);
                    let text = whisper_transcribe_utterance(&runtime, pcm).await?;
                    let elapsed = now_ms().saturating_sub(started);
                    if should_suppress_whisper_short_hallucination(&text, utterance_ms) {
                        app_log!(
                            "[whisper] rx final suppressed: seq={} elapsed_ms={} ms={} text=\"{}\"",
                            utterance_seq,
                            elapsed,
                            utterance_ms,
                            short_text(&text, 120)
                        );
                    } else {
                        app_log!(
                            "[whisper] rx final: seq={} elapsed_ms={} text=\"{}\"",
                            utterance_seq,
                            elapsed,
                            short_text(&text, 120)
                        );
                        handle_final_transcript(&event_tx, &state, text);
                    }
                    emit_status(&event_tx, "live", "Listening (offline)");
                    continue;
                }

                last_activity_ms = now_ms();
                update_audio_usage(&state, provider, model, chunk.len());
                utterance_pcm.extend_from_slice(&chunk);
            }
            _ = inactivity_check.tick() => {
                if let Some(inactivity_timeout_ms) = inactivity_timeout_ms {
                    let idle_for_ms = now_ms().saturating_sub(last_activity_ms);
                    if idle_for_ms >= inactivity_timeout_ms {
                        let _ = event_tx.send(AppEvent::SessionInactivityTimeout {
                            seconds: inactivity_timeout_secs,
                        });
                        break;
                    }
                }
            }
        }
    }

    if !utterance_pcm.is_empty() {
        utterance_seq = utterance_seq.saturating_add(1);
        let utterance_ms =
            ((utterance_pcm.len() as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
        app_log!(
            "[whisper] tx trailing utterance: seq={} bytes={} ms={}",
            utterance_seq,
            utterance_pcm.len(),
            utterance_ms
        );
        update_commit_usage(&state);
        let pcm = std::mem::take(&mut utterance_pcm);
        if let Ok(text) = whisper_transcribe_utterance(&runtime, pcm).await {
            if should_suppress_whisper_short_hallucination(&text, utterance_ms) {
                app_log!(
                    "[whisper] rx final suppressed: seq={} ms={} text=\"{}\"",
                    utterance_seq,
                    utterance_ms,
                    short_text(&text, 120)
                );
            } else {
                app_log!(
                    "[whisper] rx final: seq={} text=\"{}\"",
                    utterance_seq,
                    short_text(&text, 120)
                );
                handle_final_transcript(&event_tx, &state, text);
            }
        }
    }

    state.hotkey_recording.store(false, Ordering::SeqCst);
    Ok(())
}
