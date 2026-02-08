use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::Value;

pub struct AssemblyAiProvider;

impl SttProvider for AssemblyAiProvider {
    fn name(&self) -> &str {
        "AssemblyAI"
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let url = format!(
            "wss://streaming.assemblyai.com/v3/ws?\
             sample_rate=24000&encoding=pcm_s16le",
        );

        ConnectionConfig {
            url,
            headers: vec![
                ("Authorization".into(), settings.api_key.clone()),
                ("Host".into(), "streaming.assemblyai.com".into()),
            ],
            init_message: None,
            audio_encoding: AudioEncoding::RawBinary,
            commit_message: CommitMessage::None,
            close_message: Some(serde_json::json!({"type": "Terminate"})),
            keepalive_message: None,
            keepalive_interval_secs: 0,
            sample_rate: 24000,
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

                if transcript.is_empty() {
                    return vec![ProviderEvent::Ignore];
                }

                let end_of_turn = event
                    .get("end_of_turn")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if end_of_turn {
                    vec![ProviderEvent::TranscriptFinal(transcript.to_string())]
                } else {
                    vec![ProviderEvent::TranscriptDelta(transcript.to_string())]
                }
            }
            "Begin" => {
                let id = event
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                vec![ProviderEvent::Status(format!("session started: {}", id))]
            }
            "Termination" => vec![ProviderEvent::Status("session terminated".into())],
            "" => vec![ProviderEvent::Status(format!("unknown event: {}", event))],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }
}
