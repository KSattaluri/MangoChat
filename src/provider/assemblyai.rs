use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::Value;
use std::sync::Mutex;

/// Universal-3.x Pro models always format their output and reject
/// `format_turns`; they also accept the `mode` turn-detection preset.
fn is_pro_model(model: &str) -> bool {
    model.starts_with("universal-3-") && model.ends_with("-pro")
}

fn default_speech_model() -> &'static str {
    "universal-3-6-pro"
}

pub struct AssemblyAiProvider {
    /// turn_order of the last turn emitted as final, used to drop duplicates.
    last_final_turn: Mutex<Option<i64>>,
}

impl AssemblyAiProvider {
    pub fn new() -> Self {
        Self {
            last_final_turn: Mutex::new(None),
        }
    }
}

impl SttProvider for AssemblyAiProvider {
    fn name(&self) -> &str {
        "AssemblyAI"
    }

    fn sample_rate_hint(&self) -> u32 {
        16_000
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let model = {
            let configured = settings.assemblyai_speech_model.trim();
            if configured.is_empty() {
                default_speech_model()
            } else {
                configured
            }
        };

        let mut url = format!(
            "wss://streaming.assemblyai.com/v3/ws?\
             speech_model={}&sample_rate=16000&encoding=pcm_s16le\
             &min_turn_silence=160&max_turn_silence=1000",
            model
        );
        if is_pro_model(model) {
            // Pro models always format (format_turns is rejected) and take a
            // turn-detection preset; local VAD still forces endpoints.
            url.push_str("&mode=balanced");
        } else {
            url.push_str("&format_turns=true");
        }

        ConnectionConfig {
            url,
            headers: vec![("Authorization".into(), settings.api_key.clone())],
            init_message: None,
            audio_encoding: AudioEncoding::RawBinary,
            // Local VAD drives endpointing; ForceEndpoint closes the turn now.
            commit_message: CommitMessage::Json(serde_json::json!({"type": "ForceEndpoint"})),
            close_message: Some(serde_json::json!({"type": "Terminate"})),
            keepalive_message: None,
            keepalive_interval_secs: 0,
            // AssemblyAI expects 50-1000 ms chunks.
            min_audio_chunk_ms: 60,
            pre_commit_silence_ms: 100,
            commit_flush_timeout_ms: 700,
            sample_rate: 16000,
        }
    }

    fn parse_event(&self, text: &str) -> Vec<ProviderEvent> {
        let event: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => return vec![ProviderEvent::Error(format!("parse error: {}", e))],
        };

        let msg_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "Turn" => {
                let transcript = event
                    .get("transcript")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                if transcript.trim().is_empty() {
                    return vec![ProviderEvent::Ignore];
                }

                let end_of_turn = event
                    .get("end_of_turn")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // With format_turns=true the server sends the same turn twice:
                // unformatted first, then formatted. Only the formatted one is
                // final. Missing field is treated as formatted so a server that
                // stops sending it cannot silence finals entirely.
                let formatted = event
                    .get("turn_is_formatted")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                if !end_of_turn {
                    return vec![ProviderEvent::TranscriptDelta(transcript.to_string())];
                }
                if !formatted {
                    // The formatted copy of this turn is still coming.
                    return vec![ProviderEvent::TranscriptDelta(transcript.to_string())];
                }

                let turn_order = event.get("turn_order").and_then(|v| v.as_i64());
                if let Some(turn_order) = turn_order {
                    if let Ok(mut last) = self.last_final_turn.lock() {
                        if *last == Some(turn_order) {
                            return vec![ProviderEvent::Ignore];
                        }
                        *last = Some(turn_order);
                    }
                }
                vec![ProviderEvent::TranscriptFinal(transcript.trim().to_string())]
            }
            "Begin" => {
                // turn_order restarts at the beginning of every AssemblyAI
                // session, so the dedup state from the previous socket (this
                // provider instance is reused across reconnects) would swallow
                // the first turn of the new one.
                if let Ok(mut last) = self.last_final_turn.lock() {
                    *last = None;
                }
                let id = event
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let config = event.get("configuration");
                let model = config
                    .and_then(|c| c.get("model"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let mode = config
                    .and_then(|c| c.get("mode"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let api_version = event
                    .get("api_version")
                    .or_else(|| config.and_then(|c| c.get("api_version")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                vec![ProviderEvent::Status(format!(
                    "session started: {} (model={} mode={} api={})",
                    id, model, mode, api_version
                ))]
            }
            "Termination" => vec![ProviderEvent::Status("session terminated".into())],
            // Chatter we neither display nor act on.
            "SpeechStarted" | "Heartbeat" | "SpeakerRevision" | "LLMGatewayResponse" => {
                vec![ProviderEvent::Ignore]
            }
            "error" | "Error" => vec![ProviderEvent::Error(event.to_string())],
            "" => vec![ProviderEvent::Status(format!("unknown event: {}", event))],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(model: &str) -> ProviderSettings {
        ProviderSettings {
            api_key: "aai-test".into(),
            transcription_model: String::new(),
            language: "en".into(),
            openai_transcribe_delay: String::new(),
            assemblyai_speech_model: model.into(),
        }
    }

    #[test]
    fn url_carries_speech_model_and_current_tuning_params() {
        let provider = AssemblyAiProvider::new();
        let config = provider.connection_config(&settings("universal-streaming-english"));
        assert!(config
            .url
            .contains("speech_model=universal-streaming-english"));
        assert!(config.url.contains("format_turns=true"));
        assert!(config.url.contains("min_turn_silence=160"));
        assert!(config.url.contains("max_turn_silence=1000"));
        assert!(!config.url.contains("end_of_turn_confidence_threshold"));
        assert!(!config.url.contains("min_end_of_turn_silence_when_confident"));
        assert!(config.headers.iter().all(|(name, _)| name != "Host"));
        match config.commit_message {
            CommitMessage::Json(ref v) => assert_eq!(v["type"], "ForceEndpoint"),
            _ => panic!("expected ForceEndpoint commit"),
        }
    }

    #[test]
    fn pro_models_omit_format_turns_and_send_mode() {
        let provider = AssemblyAiProvider::new();
        for model in ["universal-3-5-pro", "universal-3-6-pro"] {
            let config = provider.connection_config(&settings(model));
            assert!(config.url.contains(&format!("speech_model={}", model)));
            assert!(!config.url.contains("format_turns"), "{}", model);
            assert!(config.url.contains("&mode=balanced"), "{}", model);
        }
        let config = provider.connection_config(&settings("universal-streaming-english"));
        assert!(!config.url.contains("mode="));
    }

    #[test]
    fn empty_speech_model_falls_back_to_default() {
        let provider = AssemblyAiProvider::new();
        let config = provider.connection_config(&settings("  "));
        assert!(config.url.contains("speech_model=universal-3-6-pro"));
    }

    #[test]
    fn only_formatted_end_of_turn_is_final() {
        let provider = AssemblyAiProvider::new();
        let partial = provider.parse_event(
            r#"{"type":"Turn","turn_order":3,"transcript":"hello wor","end_of_turn":false,"turn_is_formatted":false}"#,
        );
        assert!(matches!(
            partial.as_slice(),
            [ProviderEvent::TranscriptDelta(_)]
        ));

        let unformatted_final = provider.parse_event(
            r#"{"type":"Turn","turn_order":3,"transcript":"hello world","end_of_turn":true,"turn_is_formatted":false}"#,
        );
        assert!(matches!(
            unformatted_final.as_slice(),
            [ProviderEvent::TranscriptDelta(_)]
        ));

        let formatted_final = provider.parse_event(
            r#"{"type":"Turn","turn_order":3,"transcript":"Hello world.","end_of_turn":true,"turn_is_formatted":true}"#,
        );
        assert!(matches!(
            formatted_final.as_slice(),
            [ProviderEvent::TranscriptFinal(t)] if t == "Hello world."
        ));
    }

    #[test]
    fn duplicate_formatted_turn_is_deduped_on_turn_order() {
        let provider = AssemblyAiProvider::new();
        let raw = r#"{"type":"Turn","turn_order":7,"transcript":"Hello.","end_of_turn":true,"turn_is_formatted":true}"#;
        assert!(matches!(
            provider.parse_event(raw).as_slice(),
            [ProviderEvent::TranscriptFinal(_)]
        ));
        assert!(matches!(
            provider.parse_event(raw).as_slice(),
            [ProviderEvent::Ignore]
        ));
        let next = provider.parse_event(
            r#"{"type":"Turn","turn_order":8,"transcript":"Bye.","end_of_turn":true,"turn_is_formatted":true}"#,
        );
        assert!(matches!(
            next.as_slice(),
            [ProviderEvent::TranscriptFinal(t)] if t == "Bye."
        ));
    }

    #[test]
    fn begin_resets_turn_dedup_for_a_reconnected_session() {
        let provider = AssemblyAiProvider::new();
        let raw = r#"{"type":"Turn","turn_order":1,"transcript":"Hello.","end_of_turn":true,"turn_is_formatted":true}"#;
        assert!(matches!(
            provider.parse_event(raw).as_slice(),
            [ProviderEvent::TranscriptFinal(_)]
        ));
        // A reconnect starts a new session whose turn_order counter restarts.
        provider.parse_event(r#"{"type":"Begin","id":"new-session"}"#);
        assert!(
            matches!(
                provider.parse_event(raw).as_slice(),
                [ProviderEvent::TranscriptFinal(t)] if t == "Hello."
            ),
            "first turn after a reconnect must not be deduped away"
        );
    }

    #[test]
    fn chatter_events_are_ignored() {
        let provider = AssemblyAiProvider::new();
        for event_type in [
            "SpeechStarted",
            "Heartbeat",
            "SpeakerRevision",
            "LLMGatewayResponse",
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
    fn begin_reports_negotiated_model() {
        let provider = AssemblyAiProvider::new();
        let events = provider.parse_event(
            r#"{"type":"Begin","id":"abc","api_version":"v3","configuration":{"model":"universal-streaming-english","mode":"streaming"}}"#,
        );
        match events.as_slice() {
            [ProviderEvent::Status(msg)] => {
                assert!(msg.contains("universal-streaming-english"), "{}", msg);
                assert!(msg.contains("v3"), "{}", msg);
            }
            other => panic!("expected status, got {:?}", other),
        }
    }
}
