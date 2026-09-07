pub mod assemblyai;
pub mod deepgram;
pub mod openai;
pub mod elevenlabs;
pub mod session;

use serde_json::Value;
use std::sync::Arc;

/// Events produced by parsing a provider's WebSocket messages.
#[derive(Debug, Clone)]
pub enum ProviderEvent {
    /// Partial/interim transcript text.
    TranscriptDelta(String),
    /// Final transcript text (triggers typing).
    TranscriptFinal(String),
    /// Send a control message back through the WebSocket.
    /// Part of the provider extension API; no provider currently emits it.
    #[allow(dead_code)]
    SendControl(Value),
    /// Provider-level error.
    Error(String),
    /// Informational status (logged, not acted upon).
    Status(String),
    /// Message that should be silently ignored.
    Ignore,
}

/// How audio bytes are encoded before sending over WebSocket.
#[derive(Debug, Clone)]
pub enum AudioEncoding {
    /// Wrap base64-encoded audio in a JSON envelope.
    Base64Json {
        /// The JSON field name for the message type (e.g. "type").
        type_field: String,
        /// The value of the type field (e.g. "input_audio_buffer.append").
        type_value: String,
        /// The JSON field name for the audio payload (e.g. "audio").
        audio_field: String,
        /// Extra JSON fields to include with every audio chunk.
        extra_fields: Vec<(String, Value)>,
    },
    /// Send raw PCM bytes as a binary WebSocket frame.
    RawBinary,
}

/// What to send when the audio buffer should be committed (end of utterance).
#[derive(Debug, Clone)]
pub enum CommitMessage {
    /// Send a JSON message to commit the buffer.
    Json(Value),
    /// No commit control message; rely on provider/server-side endpointing.
    /// Part of the provider extension API; no provider currently selects it.
    #[allow(dead_code)]
    None,
}

/// Everything needed to establish and configure a provider WebSocket connection.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConnectionConfig {
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Optional JSON message to send immediately after connecting.
    pub init_message: Option<Value>,
    pub audio_encoding: AudioEncoding,
    pub commit_message: CommitMessage,
    /// Optional JSON message to send before closing the WebSocket.
    pub close_message: Option<Value>,
    /// If set, send this JSON message periodically when no audio is flowing.
    pub keepalive_message: Option<Value>,
    /// Interval in seconds for keepalive messages (default: 5).
    pub keepalive_interval_secs: u64,
    /// Minimum audio chunk duration to send, in milliseconds.
    /// 0 means send each captured chunk immediately.
    pub min_audio_chunk_ms: u32,
    /// Optional silence tail to send before commit, in milliseconds.
    /// Helps providers finalize the trailing word before endpointing.
    pub pre_commit_silence_ms: u32,
    /// Fallback delay before forcing a local flush if provider final does not arrive.
    pub commit_flush_timeout_ms: u32,
    pub sample_rate: u32,
}

/// Settings passed to a provider to build its ConnectionConfig.
#[derive(Debug, Clone)]
pub struct ProviderSettings {
    pub api_key: String,
    pub transcription_model: String,
    pub language: String,
    /// OpenAI `gpt-live-transcribe` latency knob: "" | minimal | low | medium | high | xhigh.
    pub openai_transcribe_delay: String,
    /// AssemblyAI streaming speech model (see settings::ASSEMBLYAI_SPEECH_MODELS).
    pub assemblyai_speech_model: String,
}

/// Trait that each STT provider implements.
pub trait SttProvider: Send + Sync {
    fn name(&self) -> &str;
    fn sample_rate_hint(&self) -> u32 {
        16_000
    }
    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig;
    fn parse_event(&self, text: &str) -> Vec<ProviderEvent>;
    /// Called when local VAD detects end of speech. Providers that accumulate
    /// segments (e.g. Deepgram) should flush them here as a TranscriptFinal.
    fn flush(&self) -> Vec<ProviderEvent> {
        vec![]
    }
}

/// Create a provider instance by ID.
pub fn create_provider(id: &str) -> Arc<dyn SttProvider> {
    match id {
        "deepgram" => Arc::new(deepgram::DeepgramProvider::new()),
        "elevenlabs" => Arc::new(elevenlabs::ElevenLabsProvider),
        "assemblyai" => Arc::new(assemblyai::AssemblyAiProvider::new()),
        _ => Arc::new(openai::OpenAiProvider::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deepgram's parse_event is exercised from here so deepgram.rs itself
    /// stays untouched. These assert current behavior only.
    mod deepgram_parse {
        use super::*;

        fn results(transcript: &str, is_final: bool, speech_final: bool) -> String {
            format!(
                r#"{{"type":"Results","is_final":{},"speech_final":{},"channel":{{"alternatives":[{{"transcript":"{}"}}]}}}}"#,
                is_final, speech_final, transcript
            )
        }

        #[test]
        fn interim_results_are_deltas() {
            let provider = deepgram::DeepgramProvider::new();
            let events = provider.parse_event(&results("hello", false, false));
            assert!(matches!(
                events.as_slice(),
                [ProviderEvent::TranscriptDelta(t)] if t == "hello"
            ));
        }

        #[test]
        fn interim_preview_includes_locked_in_segments() {
            let provider = deepgram::DeepgramProvider::new();
            assert!(matches!(
                provider
                    .parse_event(&results("hello", true, false))
                    .as_slice(),
                [ProviderEvent::Ignore]
            ));
            let events = provider.parse_event(&results("world", false, false));
            assert!(matches!(
                events.as_slice(),
                [ProviderEvent::TranscriptDelta(t)] if t == "hello world"
            ));
        }

        #[test]
        fn segments_accumulate_until_speech_final() {
            let provider = deepgram::DeepgramProvider::new();
            assert!(matches!(
                provider
                    .parse_event(&results("hello", true, false))
                    .as_slice(),
                [ProviderEvent::Ignore]
            ));
            let events = provider.parse_event(&results("world", true, true));
            assert!(matches!(
                events.as_slice(),
                [ProviderEvent::TranscriptFinal(t)] if t == "hello world"
            ));
        }

        #[test]
        fn utterance_end_flushes_pending_segments() {
            let provider = deepgram::DeepgramProvider::new();
            assert!(matches!(
                provider
                    .parse_event(&results("hello", true, false))
                    .as_slice(),
                [ProviderEvent::Ignore]
            ));
            let events = provider.parse_event(r#"{"type":"UtteranceEnd"}"#);
            assert!(matches!(
                events.as_slice(),
                [ProviderEvent::Status(_), ProviderEvent::TranscriptFinal(t)] if t == "hello"
            ));
            // Buffer is drained: a second UtteranceEnd emits status only.
            assert_eq!(provider.parse_event(r#"{"type":"UtteranceEnd"}"#).len(), 1);
        }

        #[test]
        fn flush_returns_nothing_when_no_segments() {
            let provider = deepgram::DeepgramProvider::new();
            assert!(provider.flush().is_empty());
        }
    }
}
