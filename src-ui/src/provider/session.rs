use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use crate::state::{AppEvent, AppState};
use crate::typing;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::sync::mpsc::Sender as EventSender;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite};

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

    request
        .body(())
        .map_err(|e| format!("Failed to build request: {}", e))
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

    let (mut ws_tx, _) = ws_stream.split();

    if let Some(ref init) = config.init_message {
        if let Err(e) = ws_tx
            .send(tungstenite::Message::Text(init.to_string().into()))
            .await
        {
            return Err(format!("{} init failed: {}", provider_name, e));
        }
    }

    let _ = ws_tx.close().await;
    Ok(())
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

pub async fn run_session(
    provider: Arc<dyn SttProvider>,
    event_tx: EventSender<AppEvent>,
    state: Arc<AppState>,
    settings: ProviderSettings,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
) {
    let config = provider.connection_config(&settings);
    let provider_name = provider.name();
    println!(
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
            emit_status(
                &event_tx,
                "error",
                &format!("Connection failed: {}", e),
            );
            return;
        }
    };
    println!("[{}] websocket connected", provider_name);

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send init message if the provider requires one.
    if let Some(ref init) = config.init_message {
        println!("[{}] sending init message", provider_name);
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
    let tx_recv = event_tx;
    let state_recv = state.clone();
    let provider_recv = provider.clone();

    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<serde_json::Value>(32);
    let (flush_tx, mut flush_rx) = mpsc::channel::<()>(8);

    let audio_encoding = config.audio_encoding.clone();
    let commit_message = config.commit_message.clone();
    let close_message = config.close_message.clone();
    let keepalive_message = config.keepalive_message.clone();
    let keepalive_secs = config.keepalive_interval_secs;
    let pname_send = provider_name.to_string();
    let activity_id = Arc::new(AtomicU64::new(0));

    // Task: forward audio from channel to WebSocket.
    let activity_id_send = activity_id.clone();
    let send_task = tokio::spawn(async move {
        let mut frames: u64 = 0;
        let mut bytes: u64 = 0;
        let keepalive_dur = if keepalive_secs > 0 {
            Duration::from_secs(keepalive_secs)
        } else {
            Duration::from_secs(3600) // effectively disabled
        };
        let mut keepalive_interval = tokio::time::interval(keepalive_dur);
        keepalive_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Skip the first immediate tick.
        keepalive_interval.tick().await;

        loop {
            tokio::select! {
                audio = audio_rx.recv() => {
                    let pcm_data = match audio {
                        Some(d) => d,
                        None => break,
                    };
                    // Empty buffer = commit signal (VAD detected end of speech).
                    if pcm_data.is_empty() {
                        println!("[{}] VAD commit", pname_send);
                        match &commit_message {
                            CommitMessage::Json(msg) => {
                                println!("[{}] sending commit message", pname_send);
                                if ws_tx
                                    .send(tungstenite::Message::Text(msg.to_string().into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            CommitMessage::None => {}
                        }
                        let commit_activity = activity_id_send.load(Ordering::SeqCst);
                        let flush_tx_delayed = flush_tx.clone();
                        let pname_flush = pname_send.clone();
                        let activity_id_flush = activity_id_send.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(700)).await;
                            if activity_id_flush.load(Ordering::SeqCst) == commit_activity {
                                println!("[{}] commit timeout flush", pname_flush);
                                let _ = flush_tx_delayed.send(()).await;
                            }
                        });
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
                        println!(
                            "[{}] audio sent: frames={}, bytes_total={}, peak={}",
                            pname_send, frames, bytes, peak
                        );
                    }

                    // Encode audio per provider config.
                    let ws_msg = match &audio_encoding {
                        AudioEncoding::Base64Json {
                            type_field,
                            type_value,
                            audio_field,
                        } => {
                            let audio_b64 = BASE64.encode(&pcm_data);
                            let msg = serde_json::json!({
                                type_field: type_value,
                                audio_field: audio_b64,
                            });
                            tungstenite::Message::Text(msg.to_string().into())
                        }
                        AudioEncoding::RawBinary => {
                            tungstenite::Message::Binary(pcm_data.into())
                        }
                    };

                    if ws_tx.send(ws_msg).await.is_err() {
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
                }
                _ = keepalive_interval.tick(), if keepalive_message.is_some() => {
                    if let Some(ref msg) = keepalive_message {
                        println!("[{}] keepalive", pname_send);
                        let _ = ws_tx
                            .send(tungstenite::Message::Text(msg.to_string().into()))
                            .await;
                    }
                }
            }
        }

        // Send close message or trailing commit before closing.
        if let Some(ref msg) = close_message {
            println!("[{}] sending close message", pname_send);
            let _ = ws_tx
                .send(tungstenite::Message::Text(msg.to_string().into()))
                .await;
        } else {
            println!("[{}] audio channel closed; sending trailing commit", pname_send);
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
        println!("[{}] closing websocket", pname_send);
        let _ = ws_tx.close().await;
    });

    let pname_recv = provider_recv.name().to_string();

    // Task: receive events from provider WebSocket.
    let recv_task = tokio::spawn(async move {
        let t0 = Instant::now();

        loop {
            let events: Vec<ProviderEvent> = tokio::select! {
                msg = ws_rx.next() => {
                    let msg = match msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            eprintln!("[{}] websocket error: {}", pname_recv, e);
                            break;
                        }
                        None => break,
                    };

                    let text = match msg {
                        tungstenite::Message::Text(t) => t,
                        tungstenite::Message::Close(frame) => {
                            if let Some(frame) = frame {
                                eprintln!(
                                    "[{}] websocket closed: {} {}",
                                    pname_recv, frame.code, frame.reason
                                );
                                emit_status(
                                    &tx_recv,
                                    "error",
                                    &format!("Disconnected: {} {}", frame.code, frame.reason),
                                );
                            } else {
                                eprintln!("[{}] websocket closed", pname_recv);
                                emit_status(&tx_recv, "error", "Disconnected");
                            }
                            break;
                        }
                        _ => continue,
                    };

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
                        println!("[{}] [{:.1}s] transcript delta: {}", pname_recv, ts, delta);
                        emit_transcript(&tx_recv, &delta, false);
                    }
                    ProviderEvent::TranscriptFinal(transcript) => {
                        println!(
                            "[{}] [{:.1}s] transcript final: \"{}\"",
                            pname_recv, ts, transcript
                        );
                        emit_transcript(&tx_recv, &transcript, true);
                        *state_recv.last_transcript.lock().unwrap() = transcript.clone();
                        let text = transcript;
                        tokio::task::spawn_blocking(move || {
                            typing::process_transcript(&text);
                        });
                    }
                    ProviderEvent::SendControl(msg) => {
                        println!("[{}] [{:.1}s] sending control message", pname_recv, ts);
                        let _ = ctrl_tx.send(msg).await;
                    }
                    ProviderEvent::Error(msg) => {
                        eprintln!("[{}] [{:.1}s] error: {}", pname_recv, ts, msg);
                        emit_status(&tx_recv, "error", &msg);
                    }
                    ProviderEvent::Status(msg) => {
                        println!("[{}] [{:.1}s] {}", pname_recv, ts, msg);
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
                println!(
                    "[{}] [{:.1}s] flush final: \"{}\"",
                    pname_recv, ts, transcript
                );
                emit_transcript(&tx_recv, &transcript, true);
                *state_recv.last_transcript.lock().unwrap() = transcript.clone();
                let text = transcript;
                tokio::task::spawn_blocking(move || {
                    typing::process_transcript(&text);
                });
            }
        }

        emit_status(&tx_recv, "idle", "Disconnected");
    });

    let _ = tokio::join!(send_task, recv_task);
    emit_status(&tx_send, "idle", "Ready");
}
