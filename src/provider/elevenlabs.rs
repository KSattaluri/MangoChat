use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::{json, Value};

/// Documented error message types. Anything in this list is surfaced to the UI.
const ERROR_MESSAGE_TYPES: &[&str] = &[
    "auth_error",
    "quota_exceeded",
    "commit_throttled",
    "rate_limited",
    "queue_overflow",
    "resource_exhausted",
    "session_time_limit_exceeded",
    "input_error",
    "invalid_request",
    "chunk_size_exceeded",
    "insufficient_audio_activity",
    "transcriber_error",
    "error",
];

fn silence_b64(sample_rate: u32, ms: u32) -> String {
    let samples = (sample_rate as u64 * ms as u64 / 1000) as usize;
    let bytes = samples * 2; // 16-bit PCM
    let buf = vec![0u8; bytes];
    BASE64.encode(buf)
}

fn error_text(event: &Value) -> String {
    for field in ["message", "description", "detail", "error"] {
        if let Some(text) = event.get(field).and_then(|v| v.as_str()) {
            if !text.trim().is_empty() {
                return text.to_string();
            }
        }
    }
    event.to_string()
}

pub struct ElevenLabsProvider;

impl SttProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "ElevenLabs Realtime"
    }

    fn sample_rate_hint(&self) -> u32 {
        16_000
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        // Use manual commit (we drive commits from local VAD).
        let mut url = "wss://api.elevenlabs.io/v1/speech-to-text/realtime\
             ?model_id=scribe_v2_realtime&commit_strategy=manual&audio_format=pcm_16000"
            .to_string();
        // Omitting language_code lets the server auto-detect.
        let language = settings.language.trim();
        if !language.is_empty() {
            url.push_str(&format!("&language_code={}", language));
        }

        let silence = silence_b64(16000, 100);
        let silence_msg = json!({
            "message_type": "input_audio_chunk",
            "audio_base_64": silence,
            "sample_rate": 16000,
        });

        ConnectionConfig {
            url,
            headers: vec![("xi-api-key".into(), settings.api_key.clone())],
            init_message: Some(silence_msg.clone()),
            audio_encoding: AudioEncoding::Base64Json {
                type_field: "message_type".into(),
                type_value: "input_audio_chunk".into(),
                audio_field: "audio_base_64".into(),
                extra_fields: vec![("sample_rate".into(), json!(16000))],
            },
            commit_message: CommitMessage::Json(json!({
                "message_type": "input_audio_chunk",
                "audio_base_64": "",
                "sample_rate": 16000,
                "commit": true,
            })),
            // "close" is not a documented client message, and setting it here
            // would make session.rs skip the trailing commit on shutdown
            // (losing buffered audio). Let the WebSocket close normally.
            close_message: None,
            keepalive_message: Some(silence_msg),
            keepalive_interval_secs: 3,
            // Docs recommend 20-250 ms audio chunks.
            min_audio_chunk_ms: 40,
            pre_commit_silence_ms: 0,
            commit_flush_timeout_ms: 700,
            sample_rate: 16000,
        }
    }

    fn parse_event(&self, text: &str) -> Vec<ProviderEvent> {
        let event: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => return vec![ProviderEvent::Error(format!("parse error: {}", e))],
        };

        let msg_type = event
            .get("message_type")
            .or_else(|| event.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        // Log raw ElevenLabs events for debugging idle/timeout behavior.
        app_log!("[ElevenLabs Realtime] event: {}", event);

        // committed_transcript, committed_transcript_with_timestamps and
        // committed_transcript_entities all carry the final text at top level.
        if msg_type.starts_with("committed_transcript") {
            let text = event.get("text").and_then(|t| t.as_str()).unwrap_or("");
            if text.trim().is_empty() {
                return vec![ProviderEvent::Ignore];
            }
            return vec![ProviderEvent::TranscriptFinal(text.to_string())];
        }
        if ERROR_MESSAGE_TYPES.contains(&msg_type) {
            return vec![ProviderEvent::Error(error_text(&event))];
        }

        match msg_type {
            "session_started" => vec![ProviderEvent::Status("session started".into())],
            "partial_transcript" => {
                let text = event.get("text").and_then(|t| t.as_str()).unwrap_or("");
                if text.is_empty() {
                    vec![ProviderEvent::Ignore]
                } else {
                    vec![ProviderEvent::TranscriptDelta(text.to_string())]
                }
            }
            "warning" => vec![ProviderEvent::Status(error_text(&event))],
            "" => vec![ProviderEvent::Error(event.to_string())],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(language: &str) -> ProviderSettings {
        ProviderSettings {
            api_key: "xi-test".into(),
            transcription_model: String::new(),
            language: language.into(),
            openai_transcribe_delay: String::new(),
            assemblyai_speech_model: String::new(),
        }
    }

    #[test]
    fn config_uses_language_and_leaves_close_to_websocket() {
        let provider = ElevenLabsProvider;
        let config = provider.connection_config(&settings("fr"));
        assert!(config.url.contains("model_id=scribe_v2_realtime"));
        assert!(config.url.contains("language_code=fr"));
        assert!(config.close_message.is_none());
        assert_eq!(config.min_audio_chunk_ms, 40);
        assert!(config.headers.iter().all(|(name, _)| name != "Host"));
    }

    #[test]
    fn empty_language_means_auto_detect() {
        let provider = ElevenLabsProvider;
        let config = provider.connection_config(&settings("  "));
        assert!(!config.url.contains("language_code"));
    }

    #[test]
    fn partial_transcript_becomes_delta() {
        let provider = ElevenLabsProvider;
        let events =
            provider.parse_event(r#"{"message_type":"partial_transcript","text":"hel"}"#);
        assert!(matches!(
            events.as_slice(),
            [ProviderEvent::TranscriptDelta(t)] if t == "hel"
        ));
    }

    #[test]
    fn all_committed_transcript_variants_are_final() {
        let provider = ElevenLabsProvider;
        for msg_type in [
            "committed_transcript",
            "committed_transcript_with_timestamps",
            "committed_transcript_entities",
        ] {
            let raw = format!(r#"{{"message_type":"{}","text":"hello"}}"#, msg_type);
            let events = provider.parse_event(&raw);
            assert!(
                matches!(events.as_slice(), [ProviderEvent::TranscriptFinal(t)] if t == "hello"),
                "{} should be final, got {:?}",
                msg_type,
                events
            );
        }
    }

    #[test]
    fn empty_committed_transcript_is_ignored() {
        let provider = ElevenLabsProvider;
        let events = provider.parse_event(r#"{"message_type":"committed_transcript","text":" "}"#);
        assert!(matches!(events.as_slice(), [ProviderEvent::Ignore]));
    }

    #[test]
    fn documented_error_types_surface_their_message() {
        let provider = ElevenLabsProvider;
        for msg_type in ERROR_MESSAGE_TYPES {
            let raw = format!(
                r#"{{"message_type":"{}","message":"boom {}"}}"#,
                msg_type, msg_type
            );
            let events = provider.parse_event(&raw);
            match events.as_slice() {
                [ProviderEvent::Error(m)] => assert!(m.contains("boom"), "{}", m),
                other => panic!("{} should be an error, got {:?}", msg_type, other),
            }
        }
    }

    #[test]
    fn error_falls_back_to_description_field() {
        let provider = ElevenLabsProvider;
        let events =
            provider.parse_event(r#"{"message_type":"auth_error","description":"bad key"}"#);
        assert!(matches!(
            events.as_slice(),
            [ProviderEvent::Error(m)] if m == "bad key"
        ));
    }

    #[test]
    fn warning_is_status_not_error() {
        let provider = ElevenLabsProvider;
        let events = provider.parse_event(r#"{"message_type":"warning","message":"slow down"}"#);
        assert!(matches!(
            events.as_slice(),
            [ProviderEvent::Status(m)] if m == "slow down"
        ));
    }
}
