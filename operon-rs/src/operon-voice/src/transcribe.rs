//! Whisper.cpp inference wrapper.

use std::path::Path;

use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn load_model(model_path: &Path) -> Result<WhisperContext> {
    eprintln!(
        "[operon-voice][transcribe] Loading whisper model from: {}",
        model_path.display()
    );

    let params = WhisperContextParameters::default();

    let ctx = WhisperContext::new_with_params(
        model_path
            .to_str()
            .context("Model path contains invalid UTF-8")?,
        params,
    )
    .map_err(|e| anyhow::anyhow!("Failed to load whisper model: {:?}", e))?;

    eprintln!("[operon-voice][transcribe] Model loaded successfully");

    Ok(ctx)
}

pub fn transcribe(ctx: &WhisperContext, audio: &[f32]) -> Result<String> {
    eprintln!(
        "[operon-voice][transcribe] Starting inference on {} samples ({:.1}s of audio)",
        audio.len(),
        audio.len() as f64 / 16_000.0
    );

    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {:?}", e))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    params.set_language(Some("en"));
    params.set_token_timestamps(false);
    params.set_translate(false);
    params.set_suppress_nst(true);

    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);

    state
        .full(params, audio)
        .map_err(|e| anyhow::anyhow!("Whisper inference failed: {:?}", e))?;

    let mut result = String::new();
    let mut segment_count = 0;

    for segment in state.as_iter() {
        segment_count += 1;
        if let Ok(text) = segment.to_str() {
            result.push_str(text);
        }
    }

    eprintln!(
        "[operon-voice][transcribe] Inference complete: {} segments, {} chars",
        segment_count,
        result.len()
    );

    Ok(result)
}
