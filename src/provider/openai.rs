use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::{json, Value};

pub struct OpenAiProvider;

impl SttProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "OpenAI Realtime"
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let url = format!(
            "wss://api.openai.com/v1/realtime?model={}",
            settings.model
        );

        let init_message = json!({
            "type": "session.update",
            "session": {
                "type": "realtime",
                "audio": {
                    "input": {
                        "format": { "type": "audio/pcm", "rate": 24000 },
                        "noise_reduction": { "type": "near_field" },
                        "transcription": {
                            "model": settings.transcription_model,
                            "language": settings.language,
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

        ConnectionConfig {
            url,
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", settings.api_key)),
                ("Host".into(), "api.openai.com".into()),
            ],
            init_message: Some(init_message),
            audio_encoding: AudioEncoding::Base64Json {
                type_field: "type".into(),
                type_value: "input_audio_buffer.append".into(),
                audio_field: "audio".into(),
                extra_fields: Vec::new(),
            },
            commit_message: CommitMessage::Json(json!({ "type": "input_audio_buffer.commit" })),
            close_message: None,
            keepalive_message: None,
            keepalive_interval_secs: 0,
            min_audio_chunk_ms: 0,
            pre_commit_silence_ms: 0,
            commit_flush_timeout_ms: 700,
            sample_rate: 24000,
        }
    }

    fn parse_event(&self, text: &str) -> Vec<ProviderEvent> {
        let event: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => return vec![ProviderEvent::Error(format!("parse error: {}", e))],
        };

        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "conversation.item.input_audio_transcription.delta" => {
                if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                    vec![ProviderEvent::TranscriptDelta(delta.to_string())]
                } else {
                    vec![ProviderEvent::Ignore]
                }
            }
            "conversation.item.input_audio_transcription.completed" => {
                let mut events = Vec::new();
                if let Some(transcript) = event.get("transcript").and_then(|t| t.as_str()) {
                    let trimmed = transcript.trim();
                    if !trimmed.is_empty() {
                        events.push(ProviderEvent::TranscriptFinal(trimmed.to_string()));
                    }
                }
                // Delete the conversation item to keep the context clean.
                if let Some(item_id) = event.get("item_id").and_then(|v| v.as_str()) {
                    events.push(ProviderEvent::SendControl(json!({
                        "type": "conversation.item.delete",
                        "item_id": item_id,
                    })));
                }
                if events.is_empty() {
                    vec![ProviderEvent::Ignore]
                } else {
                    events
                }
            }
            "error" => {
                let code = event
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                if code == "input_audio_buffer_commit_empty" {
                    return vec![ProviderEvent::Ignore];
                }
                let message = event
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("OpenAI error");
                vec![ProviderEvent::Error(message.to_string())]
            }
            "rate_limits.updated" => {
                if let Some(limits) = event.get("rate_limits").and_then(|v| v.as_array()) {
                    for limit in limits {
                        let name = limit.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                        let remaining = limit
                            .get("remaining")
                            .and_then(|r| r.as_f64())
                            .unwrap_or(0.0);
                        let limit_val =
                            limit.get("limit").and_then(|l| l.as_f64()).unwrap_or(0.0);
                        if name == "tokens" || name == "input_tokens" {
                            return vec![ProviderEvent::Status(format!(
                                "rate_limit {}: {}/{} remaining",
                                name, remaining, limit_val
                            ))];
                        }
                    }
                }
                vec![ProviderEvent::Ignore]
            }
            "" => vec![ProviderEvent::Status(format!(
                "event missing type: {}",
                event
            ))],
            _ => vec![ProviderEvent::Status(event_type.to_string())],
        }
    }
}
