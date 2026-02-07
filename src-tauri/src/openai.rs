use crate::state::AppState;
use crate::typing;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::time::sleep;
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite};

fn emit_status(app: &AppHandle, status: &str, message: &str) {
    let _ = app.emit(
        "status-update",
        json!({ "status": status, "message": message }),
    );
}

fn emit_transcript(app: &AppHandle, text: &str, is_final: bool) {
    let _ = app.emit(
        "transcript",
        json!({ "text": text, "is_final": is_final }),
    );
}

pub async fn run_session(
    app: AppHandle,
    api_key: String,
    model: String,
    transcription_model: String,
    language: String,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
) {
    let url = format!("wss://api.openai.com/v1/realtime?model={}", model);
    println!(
        "[openai] starting session: model={}, transcription_model={}, language={}",
        model, transcription_model, language
    );

    let request = tungstenite::http::Request::builder()
        .uri(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Host", "api.openai.com")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .unwrap();

    emit_status(&app, "live", "Connecting...");

    let ws_stream = match connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            emit_status(&app, "error", &format!("Connection failed: {}", e));
            return;
        }
    };
    println!("[openai] websocket connected to {}", url);

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send session.update (GA API uses nested audio.input structure)
    let session_update = json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "audio": {
                "input": {
                    "format": { "type": "audio/pcm", "rate": 24000 },
                    "noise_reduction": { "type": "near_field" },
                    "transcription": {
                        "model": transcription_model,
                        "language": language,
                    },
                    "turn_detection": {
                        "type": "server_vad",
                        "threshold": 0.5,
                        "prefix_padding_ms": 300,
                        "silence_duration_ms": 500,
                        "create_response": false,
                    },
                }
            },
        },
    });

    println!("[openai] sending session.update: {}", session_update);
    if let Err(e) = ws_tx
        .send(tungstenite::Message::Text(session_update.to_string().into()))
        .await
    {
        emit_status(&app, "error", &format!("Failed to send session.update: {}", e));
        return;
    }

    emit_status(&app, "live", "Listening");

    let app_send = app.clone();
    let app_recv = app.clone();

    // Channel for the recv_task to request outgoing WS messages (e.g. item deletes).
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<serde_json::Value>(32);

    // Task: forward audio from channel to WebSocket
    let send_task = tokio::spawn(async move {
        let mut frames: u64 = 0;
        let mut bytes: u64 = 0;
        loop {
            tokio::select! {
                audio = audio_rx.recv() => {
                    let pcm_data = match audio {
                        Some(d) => d,
                        None => break, // channel closed
                    };
                    if pcm_data.is_empty() {
                        let commit = json!({ "type": "input_audio_buffer.commit" });
                        if ws_tx
                            .send(tungstenite::Message::Text(commit.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
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
                            "[openai] audio sent: frames={}, bytes_total={}, peak={}",
                            frames, bytes, peak
                        );
                    }
                    let audio_b64 = BASE64.encode(&pcm_data);
                    let msg = json!({
                        "type": "input_audio_buffer.append",
                        "audio": audio_b64,
                    });
                    if ws_tx
                        .send(tungstenite::Message::Text(msg.to_string().into()))
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
                }
            }
        }
        // Channel closed — send commit for any trailing audio the VAD hasn't
        // auto-committed yet.
        println!("[openai] audio channel closed; sending trailing commit");
        let commit = json!({ "type": "input_audio_buffer.commit" });
        let _ = ws_tx
            .send(tungstenite::Message::Text(commit.to_string().into()))
            .await;
        // Give the server time to deliver final transcription events.
        sleep(Duration::from_millis(2000)).await;
        println!("[openai] closing websocket");
        let _ = ws_tx.close().await;
    });

    // Task: receive events from OpenAI
    let recv_task = tokio::spawn(async move {
        let t0 = Instant::now();
        while let Some(msg) = ws_rx.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[openai] websocket error: {}", e);
                    break;
                }
            };

            let text = match msg {
                tungstenite::Message::Text(t) => t,
                tungstenite::Message::Close(frame) => {
                    if let Some(frame) = frame {
                        eprintln!(
                            "[openai] websocket closed: {} {}",
                            frame.code, frame.reason
                        );
                        emit_status(
                            &app_recv,
                            "error",
                            &format!("Disconnected: {} {}", frame.code, frame.reason),
                        );
                    } else {
                        eprintln!("[openai] websocket closed");
                        emit_status(&app_recv, "error", "Disconnected");
                    }
                    break;
                }
                _ => continue,
            };

            let event: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[openai] failed to parse event: {}", e);
                    continue;
                }
            };

            let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let ts = t0.elapsed().as_secs_f32();

            match event_type {
                "conversation.item.input_audio_transcription.delta" => {
                    if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                        println!("[openai] [{:.1}s] transcript delta: {}", ts, delta);
                        emit_transcript(&app_recv, delta, false);
                    }
                }
                "conversation.item.input_audio_transcription.completed" => {
                    if let Some(transcript) =
                        event.get("transcript").and_then(|t| t.as_str())
                    {
                        let trimmed = transcript.trim();
                        println!("[openai] [{:.1}s] transcript final: \"{}\"", ts, trimmed);
                        if !trimmed.is_empty() {
                            emit_transcript(&app_recv, trimmed, true);

                            // Store last transcript
                            let state = app_recv.state::<Arc<AppState>>();
                            *state.last_transcript.lock().unwrap() = trimmed.to_string();

                            // Process for text injection
                            let text = trimmed.to_string();
                            tokio::task::spawn_blocking(move || {
                                typing::process_transcript(&text);
                            });
                        }
                    }

                    // Delete the conversation item to prevent context buildup.
                    if let Some(item_id) = event.get("item_id").and_then(|v| v.as_str()) {
                        let delete = json!({
                            "type": "conversation.item.delete",
                            "item_id": item_id,
                        });
                        println!("[openai] [{:.1}s] deleting item {}", ts, item_id);
                        let _ = ctrl_tx.send(delete).await;
                    }
                }
                "error" => {
                    let code = event
                        .get("error")
                        .and_then(|e| e.get("code"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    // Benign: trailing commit on an already-consumed VAD buffer.
                    if code == "input_audio_buffer_commit_empty" {
                        println!("[openai] [{:.1}s] ignoring empty-buffer commit", ts);
                        continue;
                    }
                    let message = event
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("OpenAI error");
                    eprintln!("[openai] [{:.1}s] error: {}", ts, message);
                    emit_status(&app_recv, "error", message);
                }
                "" => {
                    eprintln!("[openai] [{:.1}s] event missing type: {}", ts, event);
                }
                "rate_limits.updated" => {
                    if let Some(limits) = event.get("rate_limits").and_then(|v| v.as_array()) {
                        for limit in limits {
                            let name = limit.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                            let remaining = limit.get("remaining").and_then(|r| r.as_f64()).unwrap_or(0.0);
                            let limit_val = limit.get("limit").and_then(|l| l.as_f64()).unwrap_or(0.0);
                            if name == "tokens" || name == "input_tokens" {
                                println!(
                                    "[openai] [{:.1}s] rate_limit {}: {}/{} remaining",
                                    ts, name, remaining, limit_val
                                );
                            }
                        }
                    }
                }
                _ => {
                    println!("[openai] [{:.1}s] {}", ts, event_type);
                }
            }
        }

        emit_status(&app_recv, "idle", "Disconnected");
    });

    let _ = tokio::join!(send_task, recv_task);
    emit_status(&app_send, "idle", "Ready");
}


