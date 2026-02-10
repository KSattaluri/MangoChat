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
    pub model: String,
    pub transcription_model: String,
    pub language: String,
}

/// Trait that each STT provider implements.
pub trait SttProvider: Send + Sync {
    fn name(&self) -> &str;
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
        _ => Arc::new(openai::OpenAiProvider),
    }
}
