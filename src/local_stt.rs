use crate::state::{AppEvent, AppState};
use crate::typing;
use crate::whisper_runtime::{pcm16le_to_f32_mono, WhisperRuntime};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender as EventSender;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

pub const OFFLINE_SAMPLE_RATE: u32 = 16_000;

const MOONSHINE_PROVIDER_ID: &str = "moonshine";
const MOONSHINE_MODEL_LABEL: &str = "tiny-streaming-en";
const MOONSHINE_MODEL_ARCH: i32 = 2;
const WORKER_SEND_CHUNK_MS: usize = 80;
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

fn moonshine_python_path() -> PathBuf {
    repo_root().join(".venv").join("Scripts").join("python.exe")
}

fn moonshine_worker_script_path() -> PathBuf {
    repo_root().join("scripts").join("moonshine_worker.py")
}

fn moonshine_model_path() -> PathBuf {
    repo_root()
        .join(".models")
        .join("download.moonshine.ai")
        .join("model")
        .join(MOONSHINE_MODEL_LABEL)
        .join("quantized")
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

pub fn check_offline_ready(engine: &str) -> Result<(), String> {
    match engine {
        "moonshine" => {
            let python = moonshine_python_path();
            let script = moonshine_worker_script_path();
            let model = moonshine_model_path();
            if !python.exists() {
                return Err(format!(
                    "Moonshine Python runtime not found at {}",
                    python.display()
                ));
            }
            if !script.exists() {
                return Err(format!(
                    "Moonshine worker script not found at {}",
                    script.display()
                ));
            }
            if !model.exists() {
                return Err(format!(
                    "Moonshine model not found at {}",
                    model.display()
                ));
            }
            Ok(())
        }
        "whisper" => {
            let model = whisper_model_path();
            if !model.exists() {
                return Err(format!(
                    "Whisper model not found at {}",
                    model.display()
                ));
            }
            Ok(())
        }
        other => Err(format!("Unsupported offline engine: {}", other)),
    }
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
        let entry = pt.entry(MOONSHINE_PROVIDER_ID.to_string()).or_default();
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

struct MoonshineWorker {
    child: Child,
    stdin: ChildStdin,
}

impl MoonshineWorker {
    async fn spawn() -> Result<(Self, Lines<BufReader<ChildStdout>>), String> {
        let python = moonshine_python_path();
        let script = moonshine_worker_script_path();
        let model = moonshine_model_path();

        let mut child = Command::new(&python)
            .arg(&script)
            .arg("--model-path")
            .arg(&model)
            .arg("--model-arch")
            .arg(MOONSHINE_MODEL_ARCH.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start Moonshine worker: {}", e))?;
        let worker_pid = child.id().unwrap_or(0);

        let stdin = child
            .stdin
            .take()
            .ok_or("Moonshine worker stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Moonshine worker stdout unavailable")?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim();
                    if !line.is_empty() {
                        app_err!("[moonshine] {}", line);
                    }
                }
            });
        }

        let mut stdout_lines = BufReader::new(stdout).lines();
        let ready_line = stdout_lines
            .next_line()
            .await
            .map_err(|e| format!("Failed reading Moonshine startup response: {}", e))?
            .ok_or("Moonshine worker exited before signaling readiness")?;
        let ready: serde_json::Value = serde_json::from_str(&ready_line)
            .map_err(|e| format!("Invalid Moonshine startup response: {}", e))?;
        let ok = ready
            .get("type")
            .and_then(|v| v.as_str())
            .map(|v| v == "ready")
            .unwrap_or(false);
        if !ok {
            let msg = ready
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Moonshine worker failed to initialize");
            return Err(msg.to_string());
        }

        app_log!(
            "[moonshine] worker ready: pid={} model={} arch={} path={}",
            worker_pid,
            MOONSHINE_MODEL_LABEL,
            MOONSHINE_MODEL_ARCH,
            model.display()
        );

        Ok((Self { child, stdin }, stdout_lines))
    }

    async fn send_json(&mut self, value: serde_json::Value) -> Result<(), String> {
        let mut line = value.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to Moonshine worker: {}", e))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush Moonshine worker request: {}", e))
    }

    async fn send_audio_pcm16(&mut self, pcm: &[u8], sample_rate: u32) -> Result<(), String> {
        self.send_json(json!({
            "type": "audio",
            "sample_rate": sample_rate,
            "audio_b64": BASE64.encode(pcm),
        }))
        .await
    }

    async fn send_commit(&mut self) -> Result<(), String> {
        self.send_json(json!({
            "type": "commit",
        }))
        .await
    }

    async fn shutdown(mut self) {
        let _ = self.send_json(json!({ "type": "shutdown" })).await;
        let _ = tokio::time::timeout(Duration::from_millis(700), self.child.wait()).await;
        let _ = self.child.kill().await;
    }
}

fn handle_worker_line(
    line: &str,
    event_tx: &EventSender<AppEvent>,
    state: &Arc<AppState>,
    commit_seq: &Arc<AtomicU64>,
    last_commit_ms: &Arc<AtomicU64>,
) -> Result<(), String> {
    let msg: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("Invalid Moonshine event: {}", e))?;
    let current_commit = commit_seq.load(Ordering::SeqCst);
    let committed_at = last_commit_ms.load(Ordering::SeqCst);
    let since_commit_ms = if committed_at > 0 {
        now_ms().saturating_sub(committed_at)
    } else {
        0
    };
    match msg.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "delta" => {
            if let Some(text) = msg.get("text").and_then(|v| v.as_str()) {
                let text = text.trim();
                if !text.is_empty() {
                    app_log!(
                        "[moonshine] rx delta: commit={} since_commit_ms={} text=\"{}\"",
                        current_commit,
                        since_commit_ms,
                        short_text(text, 100)
                    );
                    emit_transcript(event_tx, text, false);
                }
            }
        }
        "final" => {
            if let Some(text) = msg.get("text").and_then(|v| v.as_str()) {
                app_log!(
                    "[moonshine] rx final: commit={} since_commit_ms={} text=\"{}\"",
                    current_commit,
                    since_commit_ms,
                    short_text(text, 100)
                );
                handle_final_transcript(event_tx, state, text.to_string());
            }
        }
        "status" => {
            let status = msg
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("live");
            let message = msg
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Listening (offline)");
            app_log!(
                "[moonshine] rx status: status={} message=\"{}\"",
                status,
                message
            );
            emit_status(event_tx, status, message);
        }
        "error" => {
            let message = msg
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Moonshine worker error");
            app_err!(
                "[moonshine] rx error: commit={} since_commit_ms={} message=\"{}\"",
                current_commit,
                since_commit_ms,
                message
            );
            emit_status(event_tx, "error", message);
        }
        _ => {}
    }
    Ok(())
}

pub async fn run_session(
    engine: &str,
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
    inactivity_timeout_secs: u64,
) {
    match engine {
        "moonshine" => {
            if let Err(e) =
                run_moonshine_session(event_tx.clone(), state.clone(), &mut audio_rx, inactivity_timeout_secs).await
            {
                emit_status(&event_tx, "error", &e);
            }
        }
        "whisper" => {
            if let Err(e) =
                run_whisper_session(event_tx.clone(), state.clone(), &mut audio_rx, inactivity_timeout_secs).await
            {
                emit_status(&event_tx, "error", &e);
            }
        }
        _ => {
            emit_status(&event_tx, "error", "Unknown offline engine");
        }
    }

    emit_status(&event_tx, "idle", "Ready");
}

async fn run_moonshine_session(
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    audio_rx: &mut mpsc::Receiver<Vec<u8>>,
    inactivity_timeout_secs: u64,
) -> Result<(), String> {
    let (mut worker, mut stdout_lines) = MoonshineWorker::spawn().await?;
    emit_status(&event_tx, "live", "Listening (offline)");

    let event_tx_reader = event_tx.clone();
    let state_reader = state.clone();
    let commit_seq = Arc::new(AtomicU64::new(0));
    let last_commit_ms = Arc::new(AtomicU64::new(0));
    let commit_seq_reader = commit_seq.clone();
    let last_commit_ms_reader = last_commit_ms.clone();
    let reader_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            handle_worker_line(
                &line,
                &event_tx_reader,
                &state_reader,
                &commit_seq_reader,
                &last_commit_ms_reader,
            )?;
        }
        Ok::<(), String>(())
    });

    let provider = MOONSHINE_PROVIDER_ID;
    let model = MOONSHINE_MODEL_LABEL;
    let inactivity_timeout_secs = inactivity_timeout_secs.clamp(5, 300);
    let inactivity_timeout_ms = inactivity_timeout_secs.saturating_mul(1000);
    let mut last_activity_ms = now_ms();
    let mut inactivity_check = tokio::time::interval(Duration::from_secs(1));
    inactivity_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    inactivity_check.tick().await;
    let mut has_audio_since_commit = false;
    let mut audio_chunk_seq: u64 = 0;
    let mut pending_pcm = Vec::new();
    let min_send_bytes = ((OFFLINE_SAMPLE_RATE as usize * 2 * WORKER_SEND_CHUNK_MS) / 1000).max(320);

    loop {
        tokio::select! {
            audio = audio_rx.recv() => {
                let chunk = match audio {
                    Some(c) => c,
                    None => break,
                };

                if chunk.is_empty() {
                    if !pending_pcm.is_empty() {
                        audio_chunk_seq = audio_chunk_seq.saturating_add(1);
                        let send_len = pending_pcm.len();
                        let chunk_ms =
                            ((send_len as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
                        app_log!(
                            "[moonshine] tx audio: chunk={} bytes={} ms={}",
                            audio_chunk_seq,
                            send_len,
                            chunk_ms
                        );
                        worker.send_audio_pcm16(&pending_pcm, OFFLINE_SAMPLE_RATE).await?;
                        pending_pcm.clear();
                    }
                    if has_audio_since_commit {
                        let next_commit = commit_seq.fetch_add(1, Ordering::SeqCst) + 1;
                        last_commit_ms.store(now_ms(), Ordering::SeqCst);
                        app_log!(
                            "[moonshine] tx commit: seq={}",
                            next_commit
                        );
                        update_commit_usage(&state);
                        worker.send_commit().await?;
                        has_audio_since_commit = false;
                    }
                    continue;
                }

                last_activity_ms = now_ms();
                has_audio_since_commit = true;
                update_audio_usage(&state, provider, model, chunk.len());
                pending_pcm.extend_from_slice(&chunk);
                while pending_pcm.len() >= min_send_bytes {
                    let to_send: Vec<u8> = pending_pcm.drain(..min_send_bytes).collect();
                    audio_chunk_seq = audio_chunk_seq.saturating_add(1);
                    let chunk_ms =
                        ((to_send.len() as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
                    app_log!(
                        "[moonshine] tx audio: chunk={} bytes={} ms={}",
                        audio_chunk_seq,
                        to_send.len(),
                        chunk_ms
                    );
                    worker.send_audio_pcm16(&to_send, OFFLINE_SAMPLE_RATE).await?;
                }
            }
            _ = inactivity_check.tick() => {
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

    if !pending_pcm.is_empty() {
        audio_chunk_seq = audio_chunk_seq.saturating_add(1);
        let send_len = pending_pcm.len();
        let chunk_ms =
            ((send_len as f64 / 2.0) / OFFLINE_SAMPLE_RATE as f64 * 1000.0).round() as u64;
        app_log!(
            "[moonshine] tx trailing audio: chunk={} bytes={} ms={}",
            audio_chunk_seq,
            send_len,
            chunk_ms
        );
        let _ = worker.send_audio_pcm16(&pending_pcm, OFFLINE_SAMPLE_RATE).await;
        pending_pcm.clear();
    }

    if has_audio_since_commit {
        let next_commit = commit_seq.fetch_add(1, Ordering::SeqCst) + 1;
        last_commit_ms.store(now_ms(), Ordering::SeqCst);
        app_log!(
            "[moonshine] tx trailing commit: seq={}",
            next_commit
        );
        update_commit_usage(&state);
        let _ = worker.send_commit().await;
    }

    worker.shutdown().await;

    match reader_task.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => app_err!("[moonshine] reader error: {}", e),
        Err(e) => app_err!("[moonshine] reader task join error: {}", e),
    }

    state.hotkey_recording.store(false, Ordering::SeqCst);
    Ok(())
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
    let inactivity_timeout_secs = inactivity_timeout_secs.clamp(5, 300);
    let inactivity_timeout_ms = inactivity_timeout_secs.saturating_mul(1000);
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
