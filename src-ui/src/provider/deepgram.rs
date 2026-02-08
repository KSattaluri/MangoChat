use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::{json, Value};
use std::sync::Mutex;

pub struct DeepgramProvider {
    /// Accumulates finalized segments until speech_final is true.
    segments: Mutex<Vec<String>>,
}

impl DeepgramProvider {
    pub fn new() -> Self {
        Self {
            segments: Mutex::new(Vec::new()),
        }
    }
}

impl SttProvider for DeepgramProvider {
    fn name(&self) -> &str {
        "Deepgram"
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let sample_rate = 16000;
        let url = format!(
            "wss://api.deepgram.com/v1/listen?\
             encoding=linear16&sample_rate={}&channels=1\
             &model=nova-3&language={}\
             &interim_results=true&punctuate=true\
             &endpointing=300&utterance_end_ms=1000&smart_format=true",
            sample_rate, settings.language
        );

        ConnectionConfig {
            url,
            headers: vec![
                ("Authorization".into(), format!("Token {}", settings.api_key)),
                ("Host".into(), "api.deepgram.com".into()),
            ],
            init_message: None,
            audio_encoding: AudioEncoding::RawBinary,
            commit_message: CommitMessage::Json(json!({"type": "Finalize"})),
            close_message: Some(json!({"type": "CloseStream"})),
            keepalive_message: Some(json!({"type": "KeepAlive"})),
            keepalive_interval_secs: 5,
            sample_rate,
        }
    }

    fn parse_event(&self, text: &str) -> Vec<ProviderEvent> {
        let event: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => return vec![ProviderEvent::Error(format!("parse error: {}", e))],
        };

        let msg_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "Results" => {
                let transcript = event
                    .get("channel")
                    .and_then(|c| c.get("alternatives"))
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.first())
                    .and_then(|alt| alt.get("transcript"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                let is_final = event
                    .get("is_final")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let speech_final = event
                    .get("speech_final")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !is_final {
                    // Interim result — show as delta (may change).
                    if transcript.is_empty() {
                        return vec![ProviderEvent::Ignore];
                    }
                    // Show accumulated segments + current interim for display.
                    let segments = self.segments.lock().unwrap();
                    let preview = if segments.is_empty() {
                        transcript.to_string()
                    } else {
                        format!("{} {}", segments.join(" "), transcript)
                    };
                    return vec![ProviderEvent::TranscriptDelta(preview)];
                }

                // is_final == true: this segment's text is locked in.
                if !transcript.is_empty() {
                    self.segments.lock().unwrap().push(transcript.to_string());
                }

                if speech_final {
                    // End of utterance — concatenate all accumulated segments.
                    let mut segments = self.segments.lock().unwrap();
                    let full = segments.join(" ");
                    segments.clear();
                    if full.trim().is_empty() {
                        vec![ProviderEvent::Ignore]
                    } else {
                        vec![ProviderEvent::TranscriptFinal(full)]
                    }
                } else {
                    // More segments coming for this utterance.
                    vec![ProviderEvent::Ignore]
                }
            }
            "Metadata" => vec![ProviderEvent::Status("metadata received".into())],
            "UtteranceEnd" => {
                let mut events = vec![ProviderEvent::Status("utterance end".into())];
                let flushed = self.flush();
                events.extend(flushed);
                events
            }
            "SpeechStarted" => vec![ProviderEvent::Status("speech started".into())],
            "" => vec![ProviderEvent::Status(format!("unknown event: {}", event))],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }

    fn flush(&self) -> Vec<ProviderEvent> {
        let mut segments = self.segments.lock().unwrap();
        if segments.is_empty() {
            return vec![];
        }
        let full = segments.join(" ");
        segments.clear();
        if full.trim().is_empty() {
            vec![]
        } else {
            vec![ProviderEvent::TranscriptFinal(full)]
        }
    }
}
