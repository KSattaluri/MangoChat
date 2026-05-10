use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

pub struct WhisperRuntime {
    ctx: WhisperContext,
    threads: i32,
}

impl WhisperRuntime {
    pub fn new(model_path: &Path, threads: i32) -> Result<Self, String> {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| "Whisper model path is not valid UTF-8".to_string())?;
        let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| format!("Failed to load Whisper model: {}", e))?;
        Ok(Self { ctx, threads })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("Failed to create Whisper state: {}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(self.threads);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_suppress_blank(true);
        params.set_temperature(0.0);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);

        state
            .full(params, samples)
            .map_err(|e| format!("Whisper inference failed: {}", e))?;

        let n = state.full_n_segments();
        let mut out = String::new();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                let seg_text = seg
                    .to_str_lossy()
                    .map_err(|e| format!("Whisper segment read failed: {}", e))?;
                out.push_str(seg_text.as_ref());
            }
        }
        Ok(out.trim().to_string())
    }
}

pub fn pcm16le_to_f32_mono(pcm: &[u8]) -> Vec<f32> {
    pcm.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}
