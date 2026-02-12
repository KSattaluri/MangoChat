use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::{json, Value};

fn silence_b64(sample_rate: u32, ms: u32) -> String {
    let samples = (sample_rate as u64 * ms as u64 / 1000) as usize;
    let bytes = samples * 2; // 16-bit PCM
    let buf = vec![0u8; bytes];
    BASE64.encode(buf)
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
        let url = "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime&commit_strategy=manual&audio_format=pcm_16000&language_code=en".to_string();

        let silence = silence_b64(16000, 100);
        let silence_msg = json!({
            "message_type": "input_audio_chunk",
            "audio_base_64": silence,
            "sample_rate": 16000,
        });

        ConnectionConfig {
            url,
            headers: vec![
                ("xi-api-key".into(), settings.api_key.clone()),
                ("Host".into(), "api.elevenlabs.io".into()),
            ],
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
            close_message: Some(json!({ "message_type": "close" })),
            keepalive_message: Some(silence_msg),
            keepalive_interval_secs: 3,
            min_audio_chunk_ms: 0,
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
        println!("[ElevenLabs Realtime] event: {}", event);

        match msg_type {
            "session_started" => vec![ProviderEvent::Status("session started".into())],
            "partial_transcript" => {
                let text = event
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if text.is_empty() {
                    vec![ProviderEvent::Ignore]
                } else {
                    vec![ProviderEvent::TranscriptDelta(text.to_string())]
                }
            }
            "committed_transcript" => {
                let text = event
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if text.is_empty() {
                    vec![ProviderEvent::Ignore]
                } else {
                    vec![ProviderEvent::TranscriptFinal(text.to_string())]
                }
            }
            _ if msg_type.contains("error") => {
                // Surface full error payload for debugging.
                vec![ProviderEvent::Error(event.to_string())]
            }
            "" => vec![ProviderEvent::Error(event.to_string())],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }
}
