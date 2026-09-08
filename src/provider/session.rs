use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use crate::state::{AppEvent, AppState};
use crate::typing;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use chrono::Local;
use std::sync::mpsc::Sender as EventSender;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite};

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tungstenite::Message,
>;

#[derive(Default)]
struct CommitLatencyState {
    current_commit_id: u64,
    current_commit_at: Option<Instant>,
    window_open: bool,
    first_delta_logged: bool,
    first_final_logged: bool,
}

fn build_ws_request(config: &ConnectionConfig) -> Result<tungstenite::http::Request<()>, String> {
    let mut request = tungstenite::http::Request::builder()
        .uri(&config.url)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        );

    for (name, value) in &config.headers {
        request = request.header(name.as_str(), value.as_str());
    }

    // tungstenite requires an explicit Host header on a hand-built request and
    // fails the handshake locally (before any network I/O) if it is missing.
    // Derive it from the URL unless the provider supplied one.
    let has_host = config
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"));
    if !has_host {
        let host = host_from_url(&config.url)
            .ok_or_else(|| format!("Could not derive Host from url: {}", config.url))?;
        request = request.header("Host", host);
    }

    request
        .body(())
        .map_err(|e| format!("Failed to build request: {}", e))
}

fn host_from_url(url: &str) -> Option<String> {
    let uri: tungstenite::http::Uri = url.parse().ok()?;
    let host = uri.host()?;
    Some(match uri.port_u16() {
        Some(port) => format!("{}:{}", host, port),
        None => host.to_string(),
    })
}

/// How long `validate_key` waits for the provider to say something after the
/// init message. Providers that reject a key after the HTTP upgrade (OpenAI
/// answers with an `error` event) reply well within this; providers that stay
/// silent until audio arrives are treated as valid once the window expires.
const VALIDATE_REPLY_TIMEOUT: Duration = Duration::from_secs(4);

/// Outcome of inspecting one server message during key validation.
#[derive(Debug, PartialEq)]
enum ValidationVerdict {
    /// The message proves the credentials were rejected.
    Rejected(String),
    /// The message is a normal post-connect event; the key is accepted.
    Accepted,
    /// Nothing conclusive; keep reading.
    Undecided,
}

fn classify_validation_reply(provider: &dyn SttProvider, text: &str) -> ValidationVerdict {
    let mut verdict = ValidationVerdict::Undecided;
    for event in provider.parse_event(text) {
        match event {
            ProviderEvent::Error(message) => return ValidationVerdict::Rejected(message),
            ProviderEvent::Ignore
            | ProviderEvent::Status(_)
            | ProviderEvent::TranscriptDelta(_)
            | ProviderEvent::TranscriptFinal(_)
            | ProviderEvent::SendControl(_) => verdict = ValidationVerdict::Accepted,
        }
    }
    verdict
}

pub async fn validate_key(
    provider: Arc<dyn SttProvider>,
    settings: ProviderSettings,
) -> Result<(), String> {
    let config = provider.connection_config(&settings);
    let request = build_ws_request(&config)?;
    let provider_name = provider.name();

    let ws_stream = match connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            return Err(format!("{} auth failed: {}", provider_name, e));
        }
    };

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    if let Some(ref init) = config.init_message {
        if let Err(e) = ws_tx
            .send(tungstenite::Message::Text(init.to_string().into()))
            .await
        {
            return Err(format!("{} init failed: {}", provider_name, e));
        }
    }

    // Some providers (OpenAI) accept the WebSocket upgrade for any key and
    // only report a bad credential as an event afterwards, so a successful
    // handshake alone proves nothing. Read until the server either rejects
    // the key, sends a normal event, or stays quiet for the whole window.
    let deadline = tokio::time::sleep(VALIDATE_REPLY_TIMEOUT);
    tokio::pin!(deadline);
    let result = loop {
        tokio::select! {
            _ = &mut deadline => break Ok(()),
            msg = ws_rx.next() => match msg {
                Some(Ok(tungstenite::Message::Text(text))) => {
                    match classify_validation_reply(provider.as_ref(), &text) {
                        ValidationVerdict::Rejected(message) => {
                            break Err(format!("{} rejected key: {}", provider_name, message));
                        }
                        ValidationVerdict::Accepted => break Ok(()),
                        ValidationVerdict::Undecided => {}
                    }
                }
                Some(Ok(tungstenite::Message::Close(frame))) => {
                    let detail = frame
                        .map(|f| format!("{} {}", f.code, f.reason))
                        .unwrap_or_else(|| "no close frame".into());
                    break Err(format!("{} closed the connection: {}", provider_name, detail));
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(format!("{} websocket error: {}", provider_name, e)),
                None => break Err(format!("{} closed the connection", provider_name)),
            },
        }
    };

    let _ = ws_tx.close().await;
    result
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

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn wall_ts() -> String {
    Local::now().format("%H:%M:%S%.3f").to_string()
}

const RECONNECT_BASE_MS: u64 = 800;
const RECONNECT_MAX_MS: u64 = 30_000;
const RECONNECT_MAX_RETRIES: u32 = 12;

fn reconnect_delay_ms(attempt: u32) -> u64 {
    let exp = attempt.saturating_sub(1).min(10);
    let factor = 1u64 << exp;
    (RECONNECT_BASE_MS.saturating_mul(factor)).min(RECONNECT_MAX_MS)
}

/// HTTP statuses that will never succeed on retry: bad credentials (401/403)
/// and bad request/endpoint (400/404, e.g. a removed model or parameter).
/// Retrying those just burns the backoff budget before failing anyway.
const PERMANENT_CONNECT_STATUSES: &[u16] = &[400, 401, 403, 404];

fn is_permanent_connect_error(err: &tungstenite::Error) -> bool {
    match err {
        tungstenite::Error::Http(resp) => {
            PERMANENT_CONNECT_STATUSES.contains(&resp.status().as_u16())
        }
        _ => {
            // Text fallback is kept to the pre-existing auth codes only. "400"/"404"
            // are common substrings (byte counts, ports) and a false positive here
            // would abort a healthy session with no retry.
            let text = err.to_string();
            text.contains("401") || text.contains("403")
        }
    }
}

fn provider_id_from_name(name: &str) -> String {
    match name {
        "Deepgram" => "deepgram".to_string(),
        "OpenAI Realtime" => "openai".to_string(),
        "ElevenLabs Realtime" => "elevenlabs".to_string(),
        "AssemblyAI" => "assemblyai".to_string(),
        _ => name.to_lowercase(),
    }
}

async fn send_audio_chunk(
    ws_tx: &mut WsSink,
    pcm_data: Vec<u8>,
    audio_encoding: &AudioEncoding,
    state_send: &Arc<AppState>,
    activity_ms: &Arc<AtomicU64>,
    sample_rate: u32,
    provider_name: &str,
) -> Result<(), ()> {
    if pcm_data.is_empty() {
        return Ok(());
    }

    let chunk_bytes = pcm_data.len() as u64;
    let chunk_ms = ((chunk_bytes as f64 / 2.0) / sample_rate as f64 * 1000.0) as u64;

    let ws_msg = match audio_encoding {
        AudioEncoding::Base64Json {
            type_field,
            type_value,
            audio_field,
            extra_fields,
        } => {
            let audio_b64 = BASE64.encode(&pcm_data);
            let mut map = serde_json::Map::new();
            map.insert(
                type_field.clone(),
                serde_json::Value::String(type_value.clone()),
            );
            map.insert(audio_field.clone(), serde_json::Value::String(audio_b64));
            for (key, value) in extra_fields {
                map.insert(key.clone(), value.clone());
            }
            let msg = serde_json::Value::Object(map);
            tungstenite::Message::Text(msg.to_string().into())
        }
        AudioEncoding::RawBinary => tungstenite::Message::Binary(pcm_data.into()),
    };

    if ws_tx.send(ws_msg).await.is_err() {
        return Err(());
    }
    activity_ms.store(now_ms(), Ordering::SeqCst);

    if let Ok(mut usage) = state_send.usage.lock() {
        usage.bytes_sent = usage.bytes_sent.saturating_add(chunk_bytes);
        usage.ms_sent = usage.ms_sent.saturating_add(chunk_ms);
        usage.last_update_ms = now_ms();
    }
    if let Ok(mut session) = state_send.session_usage.lock() {
        if session.started_ms != 0 {
            session.bytes_sent = session.bytes_sent.saturating_add(chunk_bytes);
            session.ms_sent = session.ms_sent.saturating_add(chunk_ms);
            session.updated_ms = now_ms();
        }
    }
    if let Ok(mut pt) = state_send.provider_totals.lock() {
        let entry = pt.entry(provider_name.to_string()).or_default();
        entry.bytes_sent = entry.bytes_sent.saturating_add(chunk_bytes);
        entry.ms_sent = entry.ms_sent.saturating_add(chunk_ms);
    }
    Ok(())
}

pub async fn run_session(
    provider: Arc<dyn SttProvider>,
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    settings: ProviderSettings,
    audio_rx: mpsc::Receiver<Vec<u8>>,
    inactivity_timeout_secs: u64,
) {
    let audio_rx = Arc::new(Mutex::new(audio_rx));
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        if attempts > 1 {
            app_log!(
                "[{}] reconnecting (attempt {})",
                provider.name(),
                attempts
            );
        }

        // If the audio channel is gone, stop.
        if audio_rx.lock().await.is_closed() {
            return;
        }

    let config = provider.connection_config(&settings);
    let provider_name = provider.name();
    let provider_id = provider_id_from_name(provider_name);
    app_log!(
        "[{}] starting session: url={}",
        provider_name, config.url
    );

    let request = match build_ws_request(&config) {
        Ok(req) => req,
        Err(e) => {
            emit_status(&event_tx, "error", &e);
            return;
        }
    };

    emit_status(&event_tx, "live", "Connecting...");

    let ws_stream = match connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            app_log!(
                "[{}] connect failed (attempt {}): {}",
                provider_name, attempts, e
            );
            if is_permanent_connect_error(&e) {
                emit_status(
                    &event_tx,
                    "error",
                    &format!("Provider rejected the connection: {}", e),
                );
                return;
            }
            if attempts >= RECONNECT_MAX_RETRIES {
                emit_status(
                    &event_tx,
                    "error",
                    &format!(
                        "Connection failed after {} retries: {}",
                        RECONNECT_MAX_RETRIES, e
                    ),
                );
                return;
            }
            let delay_ms = reconnect_delay_ms(attempts);
            emit_status(
                &event_tx,
                "error",
                &format!("Connection failed (retry {}): {}", attempts, e),
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            continue;
        }
    };
    attempts = 0;
    app_log!("[{}] websocket connected", provider_name);

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send init message if the provider requires one.
    if let Some(ref init) = config.init_message {
        app_log!("[{}] sending init message", provider_name);
        if let Err(e) = ws_tx
            .send(tungstenite::Message::Text(init.to_string().into()))
            .await
        {
            emit_status(
                &event_tx,
                "error",
                &format!("Failed to send init: {}", e),
            );
            return;
        }
    }

    emit_status(&event_tx, "live", "Listening");

    let tx_send = event_tx.clone();
    let tx_send_task = tx_send.clone();
    let tx_recv = event_tx.clone();
    let state_recv = state.clone();
    let provider_recv = provider.clone();

    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<serde_json::Value>(32);
    let (flush_tx, mut flush_rx) = mpsc::channel::<()>(8);

    let audio_encoding = config.audio_encoding.clone();
    let commit_message = config.commit_message.clone();
    let close_message = config.close_message.clone();
    let keepalive_message = config.keepalive_message.clone();
    let keepalive_secs = config.keepalive_interval_secs;
    let sample_rate = config.sample_rate.max(1);
    let min_audio_chunk_ms = config.min_audio_chunk_ms;
    let pre_commit_silence_ms = config.pre_commit_silence_ms;
    let commit_flush_timeout_ms = config.commit_flush_timeout_ms.max(100);
    let pname_send = provider_name.to_string();
    let inactivity_timeout_secs = inactivity_timeout_secs.clamp(5, 300);
    let inactivity_timeout_ms = inactivity_timeout_secs.saturating_mul(1000);
    let activity_id = Arc::new(AtomicU64::new(0));
    let last_activity_ms = Arc::new(AtomicU64::new(now_ms()));
    let commit_seq = Arc::new(AtomicU64::new(0));
    let latency_state = Arc::new(std::sync::Mutex::new(CommitLatencyState::default()));
    let state_send = state.clone();
    let provider_id_send = provider_id.clone();

    // Task: forward audio from channel to WebSocket.
    let activity_id_send = activity_id.clone();
    let last_activity_send = last_activity_ms.clone();
    let commit_seq_send = commit_seq.clone();
    let latency_state_send = latency_state.clone();
    let audio_rx_send = audio_rx.clone();
    let send_task = tokio::spawn(async move {
        let mut rx = audio_rx_send.lock().await;
        let mut timed_out = false;
        let mut frames: u64 = 0;
        let mut bytes: u64 = 0;
        let bytes_per_ms = (sample_rate as usize * 2) / 1000;
        let min_chunk_bytes = if min_audio_chunk_ms > 0 {
            (bytes_per_ms * min_audio_chunk_ms as usize).max(2)
        } else {
            0
        };
        let mut pending_pcm: Vec<u8> = Vec::new();
        let keepalive_dur = if keepalive_secs > 0 {
            Duration::from_secs(keepalive_secs)
        } else {
            Duration::from_secs(3600) // effectively disabled
        };
        let mut keepalive_interval = tokio::time::interval(keepalive_dur);
        keepalive_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Skip the first immediate tick.
        keepalive_interval.tick().await;
        let mut inactivity_check = tokio::time::interval(Duration::from_secs(1));
        inactivity_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        inactivity_check.tick().await;

        loop {
            tokio::select! {
                audio = async {
                    rx.recv().await
                } => {
                    let mut pcm_data = match audio {
                        Some(d) => d,
                        None => break,
                    };
                    // Empty buffer = commit signal (VAD detected end of speech).
                    if pcm_data.is_empty() {
                        app_log!("[{}] VAD commit", pname_send);
                        let commit_activity = activity_id_send.load(Ordering::SeqCst);
                        if !pending_pcm.is_empty() {
                            if min_chunk_bytes > 0 && pending_pcm.len() < min_chunk_bytes {
                                pending_pcm.resize(min_chunk_bytes, 0);
                            }
                            let to_send = std::mem::take(&mut pending_pcm);
                            if send_audio_chunk(
                                &mut ws_tx,
                                to_send,
                                &audio_encoding,
                                &state_send,
                                &last_activity_send,
                                sample_rate,
                                &provider_id_send,
                            )
                            .await
                            .is_err()
                            {
                                break;
                            }
                        }
                        if pre_commit_silence_ms > 0 {
                            let silence_bytes =
                                ((sample_rate as usize * 2 * pre_commit_silence_ms as usize) / 1000)
                                    .max(2);
                            let silence = vec![0u8; silence_bytes];
                            if send_audio_chunk(
                                &mut ws_tx,
                                silence,
                                &audio_encoding,
                                &state_send,
                                &last_activity_send,
                                sample_rate,
                                &provider_id_send,
                            )
                            .await
                            .is_err()
                            {
                                break;
                            }
                        }
                        match &commit_message {
                            CommitMessage::Json(msg) => {
                                app_log!("[{}] sending commit message", pname_send);
                                if ws_tx
                                    .send(tungstenite::Message::Text(msg.to_string().into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                last_activity_send.store(now_ms(), Ordering::SeqCst);
                                let commit_id = commit_seq_send.fetch_add(1, Ordering::SeqCst) + 1;
                                let committed_at = Instant::now();
                                if let Ok(mut s) = latency_state_send.lock() {
                                    s.current_commit_id = commit_id;
                                    s.current_commit_at = Some(committed_at);
                                    s.window_open = true;
                                    s.first_delta_logged = false;
                                    s.first_final_logged = false;
                                }
                                app_log!(
                                    "[{}] [{}] commit_sent id={}",
                                    pname_send,
                                    wall_ts(),
                                    commit_id
                                );
                                // Count commit
                                if let Ok(mut usage) = state_send.usage.lock() {
                                    usage.commits = usage.commits.saturating_add(1);
                                    usage.last_update_ms = now_ms();
                                }
                                if let Ok(mut session) = state_send.session_usage.lock() {
                                    if session.started_ms != 0 {
                                        session.commits = session.commits.saturating_add(1);
                                        session.updated_ms = now_ms();
                                    }
                                }
                                let flush_tx_delayed = flush_tx.clone();
                                let pname_flush = pname_send.clone();
                                let activity_id_flush = activity_id_send.clone();
                                let latency_state_flush = latency_state_send.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_millis(
                                        commit_flush_timeout_ms as u64,
                                    ))
                                    .await;
                                    if activity_id_flush.load(Ordering::SeqCst) != commit_activity {
                                        return;
                                    }
                                    let should_flush = if let Ok(mut s) = latency_state_flush.lock() {
                                        if s.window_open && s.current_commit_id == commit_id {
                                            s.window_open = false;
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };
                                    if !should_flush {
                                        return;
                                    }
                                    app_log!(
                                        "[{}] [{}] commit_timeout_flush id={} after={}ms",
                                        pname_flush,
                                        wall_ts(),
                                        commit_id,
                                        committed_at.elapsed().as_millis()
                                    );
                                    let _ = flush_tx_delayed.send(()).await;
                                });
                            }
                            CommitMessage::None => {}
                        }
                        // Don't flush locally here — let the server respond to
                        // the commit/Finalize message with speech_final or
                        // UtteranceEnd, which parse_event handles correctly.
                        // Flushing immediately races with incoming server
                        // segments and creates fragmented transcripts.
                        continue;
                    }

                    // Reset keepalive timer since we just sent real audio.
                    keepalive_interval.reset();
                    activity_id_send.fetch_add(1, Ordering::SeqCst);

                    frames += 1;
                    bytes += pcm_data.len() as u64;
                    if frames % 200 == 0 {
                        let mut peak: i32 = 0;
                        for chunk in pcm_data.chunks_exact(2) {
                            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                            let abs = (sample as i32).abs();
                            if abs > peak {
                                peak = abs;
                            }
                        }
                        app_log!(
                            "[{}] audio sent: frames={}, bytes_total={}, peak={}",
                            pname_send, frames, bytes, peak
                        );
                    }

                    if min_chunk_bytes > 0 {
                        pending_pcm.append(&mut pcm_data);
                        let mut send_failed = false;
                        while pending_pcm.len() >= min_chunk_bytes {
                            let to_send: Vec<u8> = pending_pcm.drain(..min_chunk_bytes).collect();
                            if send_audio_chunk(
                                &mut ws_tx,
                                to_send,
                                &audio_encoding,
                                &state_send,
                                &last_activity_send,
                                sample_rate,
                                &provider_id_send,
                            )
                            .await
                            .is_err()
                            {
                                send_failed = true;
                                break;
                            }
                        }
                        if send_failed {
                            break;
                        }
                    } else if send_audio_chunk(
                        &mut ws_tx,
                        pcm_data,
                        &audio_encoding,
                        &state_send,
                        &last_activity_send,
                        sample_rate,
                        &provider_id_send,
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
                ctrl = ctrl_rx.recv() => {
                    let msg = match ctrl {
                        Some(m) => m,
                        None => continue,
                    };
                    let _ = ws_tx
                        .send(tungstenite::Message::Text(msg.to_string().into()))
                        .await;
                    last_activity_send.store(now_ms(), Ordering::SeqCst);
                }
                _ = keepalive_interval.tick(), if keepalive_message.is_some() => {
                    if let Some(ref msg) = keepalive_message {
                        app_log!("[{}] keepalive", pname_send);
                        let _ = ws_tx
                            .send(tungstenite::Message::Text(msg.to_string().into()))
                            .await;
                        last_activity_send.store(now_ms(), Ordering::SeqCst);
                    }
                }
                _ = inactivity_check.tick() => {
                    let last = last_activity_send.load(Ordering::SeqCst);
                    let idle_for_ms = now_ms().saturating_sub(last);
                    if idle_for_ms >= inactivity_timeout_ms {
                        app_log!(
                            "[{}] inactivity timeout hit: {}s (idle={}ms), stopping session",
                            pname_send, inactivity_timeout_secs, idle_for_ms
                        );
                        let _ = tx_send_task.send(AppEvent::SessionInactivityTimeout {
                            seconds: inactivity_timeout_secs,
                        });
                        timed_out = true;
                        break;
                    }
                }
            }
        }

        // Send close message or trailing commit before closing.
        if let Some(ref msg) = close_message {
            app_log!("[{}] sending close message", pname_send);
            let _ = ws_tx
                .send(tungstenite::Message::Text(msg.to_string().into()))
                .await;
        } else {
            app_log!("[{}] audio channel closed; sending trailing commit", pname_send);
            match &commit_message {
                CommitMessage::Json(msg) => {
                    let _ = ws_tx
                        .send(tungstenite::Message::Text(msg.to_string().into()))
                        .await;
                }
                CommitMessage::None => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(2000)).await;
        app_log!("[{}] closing websocket", pname_send);
        let _ = ws_tx.close().await;
        timed_out
    });

    let pname_recv = provider_recv.name().to_string();
    let latency_state_recv = latency_state.clone();
    let last_activity_recv = last_activity_ms.clone();
    let provider_id_recv = provider_id.clone();

    // Task: receive events from provider WebSocket.
    let recv_task = tokio::spawn(async move {
        let t0 = Instant::now();

        loop {
            let events: Vec<ProviderEvent> = tokio::select! {
                msg = ws_rx.next() => {
                    let msg = match msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            app_err!("[{}] websocket error: {}", pname_recv, e);
                            break;
                        }
                        None => break,
                    };

                    let text = match msg {
                        tungstenite::Message::Text(t) => t,
                        tungstenite::Message::Close(frame) => {
                            if let Some(frame) = frame {
                                app_err!(
                                    "[{}] websocket closed: {} {}",
                                    pname_recv, frame.code, frame.reason
                                );
                                emit_status(
                                    &tx_recv,
                                    "error",
                                    &format!("Disconnected: {} {}", frame.code, frame.reason),
                                );
                            } else {
                                app_err!("[{}] websocket closed", pname_recv);
                                emit_status(&tx_recv, "error", "Disconnected");
                            }
                            break;
                        }
                        _ => continue,
                    };

                    last_activity_recv.store(now_ms(), Ordering::SeqCst);
                    provider_recv.parse_event(&text)
                }
                _ = flush_rx.recv() => {
                    provider_recv.flush()
                }
            };

            let ts = t0.elapsed().as_secs_f32();

            for event in events {
                match event {
                    ProviderEvent::TranscriptDelta(delta) => {
                        if let Ok(mut s) = latency_state_recv.lock() {
                            if s.window_open {
                                if let Some(start) = s.current_commit_at {
                                    let cid = s.current_commit_id;
                                    if cid > 0 && !s.first_delta_logged {
                                    app_log!(
                                        "[{}] [{}] first_delta_after_commit_ms id={} ms={}",
                                        pname_recv,
                                        wall_ts(),
                                        cid,
                                        start.elapsed().as_millis()
                                    );
                                        s.first_delta_logged = true;
                                    }
                                }
                            }
                        }
                        app_log!("[{}] [{:.1}s] transcript delta: {}", pname_recv, ts, delta);
                        emit_transcript(&tx_recv, &delta, false);
                    }
                    ProviderEvent::TranscriptFinal(transcript) => {
                        if let Ok(mut s) = latency_state_recv.lock() {
                            if s.window_open {
                                if let Some(start) = s.current_commit_at {
                                    let cid = s.current_commit_id;
                                    if cid > 0 && !s.first_final_logged {
                                        app_log!(
                                            "[{}] [{}] first_final_after_commit_ms id={} ms={}",
                                            pname_recv,
                                            wall_ts(),
                                            cid,
                                            start.elapsed().as_millis()
                                        );
                                        s.first_final_logged = true;
                                    }
                                }
                                // Close this commit window once a final is observed.
                                s.window_open = false;
                            }
                        }
                        app_log!(
                            "[{}] [{:.1}s] transcript final: \"{}\"",
                            pname_recv, ts, transcript
                        );
                        emit_transcript(&tx_recv, &transcript, true);
                        if let Ok(mut usage) = state_recv.usage.lock() {
                            usage.finals = usage.finals.saturating_add(1);
                        }
                        if let Ok(mut session) = state_recv.session_usage.lock() {
                            if session.started_ms != 0 {
                                session.finals = session.finals.saturating_add(1);
                            }
                        }
                        if let Ok(mut pt) = state_recv.provider_totals.lock() {
                            let entry = pt.entry(provider_id_recv.clone()).or_default();
                            entry.finals = entry.finals.saturating_add(1);
                        }
                        if let Ok(mut last) = state_recv.last_transcript.lock() {
                            *last = transcript.clone();
                        }
                        let chrome = state_recv.chrome_path.lock().ok().map(|g| g.clone()).unwrap_or_default();
                        let paint = state_recv.paint_path.lock().ok().map(|g| g.clone()).unwrap_or_default();
                        let urls = state_recv.url_commands.lock().ok().map(|g| g.clone()).unwrap_or_default();
                        let aliases = state_recv.alias_commands.lock().ok().map(|g| g.clone()).unwrap_or_default();
                        let apps = state_recv.app_shortcuts.lock().ok().map(|g| g.clone()).unwrap_or_default();
                        let text = transcript;
                        tokio::task::spawn_blocking(move || {
                            typing::process_transcript(&text, &chrome, &paint, &urls, &aliases, &apps);
                        });
                    }
                    ProviderEvent::SendControl(msg) => {
                        app_log!("[{}] [{:.1}s] sending control message", pname_recv, ts);
                        let _ = ctrl_tx.send(msg).await;
                    }
                    ProviderEvent::Error(msg) => {
                        app_err!("[{}] [{:.1}s] error: {}", pname_recv, ts, msg);
                        emit_status(&tx_recv, "error", &msg);
                    }
                    ProviderEvent::Status(msg) => {
                        app_log!("[{}] [{:.1}s] {}", pname_recv, ts, msg);
                    }
                    ProviderEvent::Ignore => {}
                }
            }
        }

        // Flush any remaining segments on disconnect.
        let remaining = provider_recv.flush();
        for event in remaining {
            if let ProviderEvent::TranscriptFinal(transcript) = event {
                let ts = t0.elapsed().as_secs_f32();
                app_log!(
                    "[{}] [{:.1}s] flush final: \"{}\"",
                    pname_recv, ts, transcript
                );
                emit_transcript(&tx_recv, &transcript, true);
                if let Ok(mut usage) = state_recv.usage.lock() {
                    usage.finals = usage.finals.saturating_add(1);
                }
                if let Ok(mut session) = state_recv.session_usage.lock() {
                    if session.started_ms != 0 {
                        session.finals = session.finals.saturating_add(1);
                    }
                }
                if let Ok(mut pt) = state_recv.provider_totals.lock() {
                    let entry = pt.entry(provider_id_recv.clone()).or_default();
                    entry.finals = entry.finals.saturating_add(1);
                }
                if let Ok(mut last) = state_recv.last_transcript.lock() {
                    *last = transcript.clone();
                }
                let chrome = state_recv.chrome_path.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let paint = state_recv.paint_path.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let urls = state_recv.url_commands.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let aliases = state_recv.alias_commands.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let apps = state_recv.app_shortcuts.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let text = transcript;
                tokio::task::spawn_blocking(move || {
                    typing::process_transcript(&text, &chrome, &paint, &urls, &aliases, &apps);
                });
            }
        }

        emit_status(&tx_recv, "idle", "Disconnected");
    });

    let (send_result, _) = tokio::join!(send_task, recv_task);
    let timed_out = send_result.unwrap_or(false);
    if timed_out {
        return;
    }
    emit_status(&tx_send, "idle", "Ready");
    // Retry unless audio channel is closed.
    if audio_rx.lock().await.is_closed() {
        return;
    }
    tokio::time::sleep(Duration::from_millis(RECONNECT_BASE_MS)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_error(status: u16) -> tungstenite::Error {
        let response = tungstenite::http::Response::builder()
            .status(status)
            .body(None)
            .expect("response");
        tungstenite::Error::Http(response)
    }

    #[test]
    fn auth_and_bad_request_statuses_are_permanent() {
        for status in [400, 401, 403, 404] {
            assert!(
                is_permanent_connect_error(&http_error(status)),
                "{} should be permanent",
                status
            );
        }
    }

    fn config_with_headers(url: &str, headers: Vec<(String, String)>) -> ConnectionConfig {
        ConnectionConfig {
            url: url.to_string(),
            headers,
            init_message: None,
            audio_encoding: AudioEncoding::RawBinary,
            commit_message: CommitMessage::None,
            close_message: None,
            keepalive_message: None,
            keepalive_interval_secs: 0,
            min_audio_chunk_ms: 0,
            pre_commit_silence_ms: 0,
            commit_flush_timeout_ms: 0,
            sample_rate: 16_000,
        }
    }

    /// Live check against the real providers. Run with:
    ///   MANGOCHAT_LIVE_KEY_<PROVIDER>=<key> cargo test live_validate -- --ignored --nocapture
    /// Any provider without an env var is skipped. A key of "bad" is expected
    /// to be rejected; anything else is expected to be accepted.
    #[test]
    #[ignore]
    fn live_validate_key_reads_server_verdict() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let mut failures = Vec::new();
        for id in ["openai", "deepgram", "assemblyai", "elevenlabs"] {
            let var = format!("MANGOCHAT_LIVE_KEY_{}", id.to_uppercase());
            let Ok(key) = std::env::var(&var) else { continue };
            let expect_ok = key != "bad";
            let settings = ProviderSettings {
                api_key: key,
                transcription_model: "gpt-transcribe".into(),
                language: "en".into(),
                openai_transcribe_delay: String::new(),
                assemblyai_speech_model: String::new(),
            };
            let started = Instant::now();
            let result = rt.block_on(validate_key(super::super::create_provider(id), settings));
            eprintln!(
                "{}: expect_ok={} took={:?} result={:?}",
                id,
                expect_ok,
                started.elapsed(),
                result
            );
            if result.is_ok() != expect_ok {
                failures.push(id);
            }
        }
        assert!(failures.is_empty(), "wrong verdict for {:?}", failures);
    }

    #[test]
    fn validation_reply_error_event_is_rejected() {
        let provider = super::super::openai::OpenAiProvider::new();
        let verdict = classify_validation_reply(
            &provider,
            r#"{"type":"error","error":{"code":"invalid_api_key","message":"Incorrect API key provided"}}"#,
        );
        assert_eq!(
            verdict,
            ValidationVerdict::Rejected("Incorrect API key provided".into())
        );
    }

    #[test]
    fn validation_reply_lifecycle_event_is_accepted() {
        let provider = super::super::openai::OpenAiProvider::new();
        let verdict = classify_validation_reply(&provider, r#"{"type":"session.created"}"#);
        assert_eq!(verdict, ValidationVerdict::Accepted);
    }

    #[test]
    fn validation_reply_unparseable_text_is_rejected() {
        // parse_event reports unparseable input as a provider error; a server
        // that sends garbage after the init message is not a working session.
        let provider = super::super::openai::OpenAiProvider::new();
        let verdict = classify_validation_reply(&provider, "not json");
        assert!(matches!(verdict, ValidationVerdict::Rejected(_)));
    }

    #[test]
    fn ws_request_derives_host_from_url_when_not_supplied() {
        let config = config_with_headers(
            "wss://api.openai.com/v1/realtime?intent=transcription",
            vec![("Authorization".into(), "Bearer x".into())],
        );
        let request = build_ws_request(&config).expect("request");
        assert_eq!(request.headers().get("host").unwrap(), "api.openai.com");
        assert_eq!(request.headers().get("authorization").unwrap(), "Bearer x");
    }

    #[test]
    fn ws_request_keeps_provider_supplied_host() {
        let config = config_with_headers(
            "wss://api.deepgram.com/v1/listen",
            vec![("Host".into(), "api.deepgram.com".into())],
        );
        let request = build_ws_request(&config).expect("request");
        let hosts: Vec<_> = request.headers().get_all("host").iter().collect();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0], "api.deepgram.com");
    }

    #[test]
    fn host_from_url_includes_explicit_port() {
        assert_eq!(host_from_url("wss://localhost:8765/ws").as_deref(), Some("localhost:8765"));
        assert_eq!(host_from_url("wss://streaming.assemblyai.com/v3/ws?x=1").as_deref(), Some("streaming.assemblyai.com"));
        assert_eq!(host_from_url("not a url"), None);
    }

    #[test]
    fn text_fallback_only_matches_auth_codes() {
        let io = |msg: &str| tungstenite::Error::Io(std::io::Error::other(msg.to_string()));
        assert!(is_permanent_connect_error(&io("server replied 401 Unauthorized")));
        assert!(is_permanent_connect_error(&io("403 Forbidden")));
        // "400"/"404" as loose substrings must NOT abort a session.
        assert!(!is_permanent_connect_error(&io("read 40400 bytes before reset")));
        assert!(!is_permanent_connect_error(&io("connection reset on port 8400")));
    }

    #[test]
    fn transient_statuses_are_retried() {
        for status in [429, 500, 502, 503] {
            assert!(
                !is_permanent_connect_error(&http_error(status)),
                "{} should be retried",
                status
            );
        }
    }

    #[test]
    fn reconnect_delay_grows_and_is_capped() {
        assert_eq!(reconnect_delay_ms(1), RECONNECT_BASE_MS);
        assert_eq!(reconnect_delay_ms(2), RECONNECT_BASE_MS * 2);
        assert_eq!(reconnect_delay_ms(20), RECONNECT_MAX_MS);
    }
}
