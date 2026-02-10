use super::{
    AudioEncoding, CommitMessage, ConnectionConfig, ProviderEvent, ProviderSettings, SttProvider,
};
use serde_json::json;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
struct AssemblyState {
    last_final_norm: String,
    last_final_turn_order: Option<i64>,
    pending_partial: String,
    pending_partial_at: Option<Instant>,
}

pub struct AssemblyAiProvider {
    state: Mutex<AssemblyState>,
}

impl AssemblyAiProvider {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AssemblyState::default()),
        }
    }
}

fn normalize_for_dedupe(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl SttProvider for AssemblyAiProvider {
    fn name(&self) -> &str {
        "AssemblyAI"
    }

    fn connection_config(&self, settings: &ProviderSettings) -> ConnectionConfig {
        let url = format!(
            "wss://streaming.assemblyai.com/v3/ws?\
             sample_rate=16000&encoding=pcm_s16le\
             &format_turns=false\
             &end_of_turn_confidence_threshold=0.42\
             &min_end_of_turn_silence_when_confident=260\
             &max_turn_silence=500",
        );

        ConnectionConfig {
            url,
            headers: vec![
                ("Authorization".into(), settings.api_key.clone()),
                ("Host".into(), "streaming.assemblyai.com".into()),
            ],
            init_message: None,
            audio_encoding: AudioEncoding::RawBinary,
            // Local VAD commit should force server turn finalization.
            commit_message: CommitMessage::Json(json!({"type": "ForceEndpoint"})),
            close_message: Some(json!({"type": "Terminate"})),
            keepalive_message: None,
            keepalive_interval_secs: 0,
            // AssemblyAI expects 50-1000 ms chunks.
            min_audio_chunk_ms: 60,
            // Send a short silence tail before endpointing to improve trailing-word finalization.
            pre_commit_silence_ms: 70,
            // Faster fallback for single short words after a pause.
            commit_flush_timeout_ms: 380,
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

                if transcript.is_empty() {
                    return vec![ProviderEvent::Ignore];
                }

                let end_of_turn = event
                    .get("end_of_turn")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let turn_order = event
                    .get("turn_order")
                    .and_then(|v| v.as_i64());
                let utterance = event
                    .get("utterance")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .trim();

                // Per AssemblyAI message sequence docs:
                // utterance can be finalized while end_of_turn is still false.
                if !utterance.is_empty() {
                    let norm = normalize_for_dedupe(utterance);
                    let mut st = self.state.lock().unwrap();
                    let same_turn = st
                        .last_final_turn_order
                        .zip(turn_order)
                        .map(|(a, b)| a == b)
                        .unwrap_or(false);
                    let is_duplicate = same_turn && st.last_final_norm == norm;
                    if is_duplicate {
                        return vec![ProviderEvent::Ignore];
                    }
                    st.pending_partial.clear();
                    st.pending_partial_at = None;
                    st.last_final_norm = norm;
                    st.last_final_turn_order = turn_order;
                    return vec![ProviderEvent::TranscriptFinal(utterance.to_string())];
                }

                if end_of_turn {
                    let trimmed = transcript.trim();
                    if trimmed.is_empty() {
                        return vec![ProviderEvent::Ignore];
                    }

                    let norm = normalize_for_dedupe(trimmed);
                    let mut st = self.state.lock().unwrap();
                    let same_turn = st
                        .last_final_turn_order
                        .zip(turn_order)
                        .map(|(a, b)| a == b)
                        .unwrap_or(false);
                    let is_duplicate = same_turn && st.last_final_norm == norm;
                    if is_duplicate {
                        vec![ProviderEvent::Ignore]
                    } else {
                        st.pending_partial.clear();
                        st.pending_partial_at = None;
                        st.last_final_norm = norm;
                        st.last_final_turn_order = turn_order;
                        vec![ProviderEvent::TranscriptFinal(trimmed.to_string())]
                    }
                } else {
                    let trimmed = transcript.trim();
                    if trimmed.is_empty() {
                        return vec![ProviderEvent::Ignore];
                    }
                    let mut st = self.state.lock().unwrap();
                    st.pending_partial = trimmed.to_string();
                    st.pending_partial_at = Some(Instant::now());
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
            "error" | "Error" => vec![ProviderEvent::Error(event.to_string())],
            "" => vec![ProviderEvent::Status(format!("unknown event: {}", event))],
            _ => vec![ProviderEvent::Status(msg_type.to_string())],
        }
    }

    fn flush(&self) -> Vec<ProviderEvent> {
        let mut st = self.state.lock().unwrap();
        let pending = st.pending_partial.trim();
        if pending.is_empty() {
            return vec![];
        }
        let fresh = st
            .pending_partial_at
            .map(|t| t.elapsed() <= Duration::from_secs(3))
            .unwrap_or(false);
        if !fresh {
            st.pending_partial.clear();
            st.pending_partial_at = None;
            return vec![];
        }

        let norm = normalize_for_dedupe(pending);
        let is_duplicate = st.last_final_norm == norm;
        if is_duplicate {
            st.pending_partial.clear();
            st.pending_partial_at = None;
            return vec![];
        }

        let text = pending.to_string();
        st.pending_partial.clear();
        st.pending_partial_at = None;
        st.last_final_norm = norm;
        st.last_final_turn_order = None;
        vec![ProviderEvent::TranscriptFinal(text)]
    }
}
