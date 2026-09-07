use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::{json, Value};
use std::sync::Mutex;

/// Realtime *transcription session* endpoint.
///
/// NOTE (2026-09-07): the `?intent=transcription` query string is
/// community-established, not documented by OpenAI. A transcription session
/// takes no speech-to-speech model, so there is no `model=` parameter here.
/// If the handshake fails, a tester should try these fallbacks in order:
///   1. bare `wss://api.openai.com/v1/realtime`
///   2. `wss://api.openai.com/v1/realtime?model=gpt-transcribe`
/// Kept in one const so swapping it is a one-line change.
const REALTIME_TRANSCRIPTION_URL: &str = "wss://api.openai.com/v1/realtime?intent=transcription";

/// Transcription model that supports the `delay` latency knob.
const LIVE_TRANSCRIBE_MODEL: &str = "gpt-live-transcribe";

pub struct OpenAiProvider {
    /// item_id of the last completed transcript, used to drop duplicates.
    last_completed_item: Mutex<Option<String>>,
}

impl OpenAiProvider {
    pub fn new() -> Self {
        Self {
            last_completed_item: Mutex::new(None),
        }
    }
}

impl SttProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "OpenAI Realtime"
    }

    fn sample_rate_hint(&self) -> u32 {
        24_000
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let mut transcription = json!({ "model": settings.transcription_model });
        let language = settings.language.trim();
        if !language.is_empty() {
            transcription["language"] = json!(language);
        }
        let delay = settings.openai_transcribe_delay.trim();
        if settings.transcription_model == LIVE_TRANSCRIBE_MODEL && !delay.is_empty() {
            transcription["delay"] = json!(delay);
        }

        // Transcription sessions have no server VAD / noise reduction knobs:
        // local VAD drives input_audio_buffer.commit instead.
        let init_message = json!({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": { "type": "audio/pcm", "rate": 24000 },
                        "transcription": transcription,
                        "turn_detection": Value::Null,
                    }
                },
            },
        });

        ConnectionConfig {
            url: REALTIME_TRANSCRIPTION_URL.to_string(),
            headers: vec![(
                "Authorization".into(),
                format!("Bearer {}", settings.api_key),
            )],
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
                // The server can repeat a completed event for the same item;
                // emit the transcript only once.
                let item_id = event.get("item_id").and_then(|v| v.as_str());
                if let Some(item_id) = item_id {
                    if let Ok(last) = self.last_completed_item.lock() {
                        if last.as_deref() == Some(item_id) {
                            return vec![ProviderEvent::Ignore];
                        }
                    }
                }
                let transcript = event
                    .get("transcript")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .trim();
                if transcript.is_empty() {
                    // Don't record the id: an empty completed must not shadow a
                    // later real transcript for the same item.
                    return vec![ProviderEvent::Ignore];
                }
                if let Some(item_id) = item_id {
                    if let Ok(mut last) = self.last_completed_item.lock() {
                        *last = Some(item_id.to_string());
                    }
                }
                vec![ProviderEvent::TranscriptFinal(transcript.to_string())]
            }
            "conversation.item.input_audio_transcription.failed" => {
                let message = event
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("transcription failed");
                vec![ProviderEvent::Error(message.to_string())]
            }
            // Session/buffer lifecycle chatter we neither display nor act on.
            "session.created"
            | "session.updated"
            | "input_audio_buffer.committed"
            | "input_audio_buffer.speech_started"
            | "input_audio_buffer.speech_stopped"
            | "input_audio_buffer.cleared"
            | "conversation.item.added"
            | "conversation.item.done"
            | "conversation.item.created" => vec![ProviderEvent::Ignore],
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

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(model: &str, language: &str, delay: &str) -> ProviderSettings {
        ProviderSettings {
            api_key: "sk-test".into(),
            transcription_model: model.into(),
            language: language.into(),
            openai_transcribe_delay: delay.into(),
            assemblyai_speech_model: String::new(),
        }
    }

    #[test]
    fn config_uses_transcription_session_without_s2s_model() {
        let provider = OpenAiProvider::new();
        let config = provider.connection_config(&settings("gpt-transcribe", "en", ""));
        assert_eq!(config.url, REALTIME_TRANSCRIPTION_URL);
        assert!(!config.url.contains("model="));
        assert!(config.headers.iter().all(|(name, _)| name != "Host"));
        let init = config.init_message.expect("init message");
        assert_eq!(init["session"]["type"], "transcription");
        let input = &init["session"]["audio"]["input"];
        assert_eq!(input["transcription"]["model"], "gpt-transcribe");
        assert_eq!(input["transcription"]["language"], "en");
        assert!(input["turn_detection"].is_null());
        assert!(input.get("noise_reduction").is_none());
    }

    #[test]
    fn config_omits_language_and_delay_when_not_applicable() {
        let provider = OpenAiProvider::new();
        let config = provider.connection_config(&settings("gpt-transcribe", "  ", "minimal"));
        let init = config.init_message.expect("init message");
        let transcription = &init["session"]["audio"]["input"]["transcription"];
        assert!(transcription.get("language").is_none());
        // delay only applies to gpt-live-transcribe
        assert!(transcription.get("delay").is_none());
    }

    #[test]
    fn config_includes_delay_for_live_transcribe() {
        let provider = OpenAiProvider::new();
        let config = provider.connection_config(&settings("gpt-live-transcribe", "en", "minimal"));
        let init = config.init_message.expect("init message");
        assert_eq!(
            init["session"]["audio"]["input"]["transcription"]["delay"],
            "minimal"
        );
    }

    #[test]
    fn delta_event_becomes_transcript_delta() {
        let provider = OpenAiProvider::new();
        let events = provider.parse_event(
            r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"i1","delta":"hel"}"#,
        );
        assert!(matches!(
            events.as_slice(),
            [ProviderEvent::TranscriptDelta(t)] if t == "hel"
        ));
    }

    #[test]
    fn completed_event_becomes_final_without_item_delete() {
        let provider = OpenAiProvider::new();
        let events = provider.parse_event(
            r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"i1","transcript":" hello world "}"#,
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events.as_slice(),
            [ProviderEvent::TranscriptFinal(t)] if t == "hello world"
        ));
    }

    #[test]
    fn duplicate_completed_event_is_ignored() {
        let provider = OpenAiProvider::new();
        let raw = r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"i1","transcript":"hello"}"#;
        let first = provider.parse_event(raw);
        assert!(matches!(
            first.as_slice(),
            [ProviderEvent::TranscriptFinal(_)]
        ));
        let second = provider.parse_event(raw);
        assert!(matches!(second.as_slice(), [ProviderEvent::Ignore]));
    }

    #[test]
    fn lifecycle_events_are_ignored() {
        let provider = OpenAiProvider::new();
        for event_type in [
            "session.created",
            "session.updated",
            "input_audio_buffer.committed",
            "input_audio_buffer.speech_started",
            "input_audio_buffer.speech_stopped",
            "conversation.item.added",
            "conversation.item.done",
            "conversation.item.created",
        ] {
            let raw = format!(r#"{{"type":"{}"}}"#, event_type);
            let events = provider.parse_event(&raw);
            assert!(
                matches!(events.as_slice(), [ProviderEvent::Ignore]),
                "{} should be ignored, got {:?}",
                event_type,
                events
            );
        }
    }

    #[test]
    fn empty_commit_error_is_ignored_but_others_surface() {
        let provider = OpenAiProvider::new();
        let ignored = provider
            .parse_event(r#"{"type":"error","error":{"code":"input_audio_buffer_commit_empty"}}"#);
        assert!(matches!(ignored.as_slice(), [ProviderEvent::Ignore]));

        let surfaced = provider.parse_event(
            r#"{"type":"error","error":{"code":"invalid_request_error","message":"bad model"}}"#,
        );
        assert!(matches!(
            surfaced.as_slice(),
            [ProviderEvent::Error(m)] if m == "bad model"
        ));
    }
}
