//! # operon-voice
//!
//! Local, fully offline voice-to-text dictation engine.
//!
//! This crate captures audio from the system microphone via `cpal`, runs
//! energy-based voice activity detection (VAD), and feeds recorded audio
//! chunks into `whisper.cpp` (via the `whisper-rs` bindings) for transcription.
//!
//! ## Architecture
//!
//! ```text
//!   ┌──────────┐     ┌──────────┐     ┌──────────────┐
//!   │  cpal    │────▶│   VAD    │────▶│  whisper.cpp  │
//!   │ capture  │     │ silence  │     │  inference    │
//!   └──────────┘     │ detector │     └──────┬───────┘
//!                    └──────────┘            │
//!                                    VoiceEvent channel
//! ```
//!
//! ## Privacy
//!
//! No network calls, no telemetry, no cloud APIs. All processing runs on the
//! local CPU. Audio never leaves the process.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use operon_voice::{VoiceEngine, VoiceEvent};
//! use std::path::PathBuf;
//!
//! let engine = VoiceEngine::new(PathBuf::from("~/.operon/models/ggml-tiny.en.bin"));
//! let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
//! // engine.start(tx).await?;
//! // ... receive VoiceEvents from rx ...
//! // engine.stop().await?;
//! ```

// Submodule declarations — each one handles a distinct concern
pub mod capture;    // cpal microphone capture → f32 PCM buffer
pub mod transcribe; // whisper-rs model loading + inference
pub mod vad;        // Energy-based voice activity / silence detection

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Public types — these are re-exported through the operon-rs facade
// ─────────────────────────────────────────────────────────────────────────────

/// The four states the voice engine cycles through.
///
/// The GUI binds to this (as an integer on the Slint side) to drive visual
/// transitions: mic icon → spinner → waveform → spinner → mic icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    /// No recording in progress. The button shows the static mic icon.
    Idle = 0,
    /// The whisper model is being loaded from disk into RAM.
    /// The button shows a spinning loader.
    Loading = 1,
    /// Microphone is actively capturing audio. The button shows the waveform
    /// animation. VAD is running to detect end-of-speech.
    Listening = 2,
    /// Audio capture has stopped and whisper inference is running on the
    /// buffered audio. The button shows the spinning loader again.
    Transcribing = 3,
}

/// Events emitted by the voice engine to the GUI event loop.
///
/// These flow through a `tokio::sync::mpsc::UnboundedSender` so the GUI can
/// update Slint properties via `slint::invoke_from_event_loop`.
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// The engine has transitioned to a new state.
    StateChanged(VoiceState),
    /// A partial (intermediate) transcript is available. Currently unused
    /// because whisper.cpp doesn't support true incremental streaming, but
    /// the variant is here for future compatibility.
    PartialTranscript(String),
    /// The final transcript for the recorded audio chunk.
    FinalTranscript(String),
    /// Something went wrong (mic access denied, model file missing, etc.).
    /// The GUI should log this and reset to Idle.
    Error(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// VoiceEngine — the main orchestrator
// ─────────────────────────────────────────────────────────────────────────────

/// Orchestrates the full voice-to-text pipeline:
/// mic capture → VAD → whisper inference → transcript.
///
/// The engine is designed to be created once and reused across multiple
/// start/stop cycles. Each `start()` loads the model, each `stop()` drops it
/// to reclaim RAM.
pub struct VoiceEngine {
    /// Path to the GGML model file (e.g. `~/.operon/models/ggml-tiny.en.bin`).
    model_path: PathBuf,

    /// Signal flag: when set to `true`, the capture/VAD loop should stop.
    /// Protected by an async Mutex so both the event loop and the stop()
    /// caller can access it without blocking the Slint UI thread.
    stop_signal: Arc<Mutex<bool>>,

    /// Handle to the cpal audio stream. Held as `Option` so we can take/drop
    /// it on stop without requiring `&mut self`.
    capture_handle: Arc<Mutex<Option<capture::CaptureHandle>>>,
}

impl VoiceEngine {
    /// Create a new engine pointing at the given model file.
    ///
    /// This does NOT load the model or open the microphone — those happen
    /// lazily inside `start()`.
    pub fn new(model_path: PathBuf) -> Self {
        Self {
            model_path,
            stop_signal: Arc::new(Mutex::new(false)),
            capture_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Begin the voice capture + transcription pipeline.
    ///
    /// Flow:
    /// 1. Emit `StateChanged(Loading)` — GUI shows spinner.
    /// 2. Load whisper model via `spawn_blocking` (CPU-bound, ~200ms cached).
    /// 3. Open microphone via cpal.
    /// 4. Emit `StateChanged(Listening)` — GUI shows waveform animation.
    /// 5. Accumulate audio, run VAD. When silence detected OR `stop()` called:
    /// 6. Emit `StateChanged(Transcribing)` — GUI shows spinner.
    /// 7. Run whisper inference on buffered audio.
    /// 8. Emit `FinalTranscript(text)`.
    /// 9. Drop the whisper context to release RAM.
    /// 10. Emit `StateChanged(Idle)`.
    pub async fn start(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<VoiceEvent>,
    ) -> anyhow::Result<()> {
        // Reset the stop signal for this new recording session
        {
            let mut stop = self.stop_signal.lock().await;
            *stop = false;
        }

        // ── Step 1: Validate model path ──────────────────────────────────
        let model_path = self.model_path.clone();
        if !model_path.exists() {
            let msg = format!(
                "Whisper model not found at: {}\n\
                 Please download the model and place it at the path above.\n\
                 You can get ggml-tiny.en.bin from:\n\
                 https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
                model_path.display()
            );
            eprintln!("[operon-voice] {}", msg);
            let _ = tx.send(VoiceEvent::Error(msg));
            let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Idle));
            return Ok(());
        }

        // ── Step 2: Load model (blocking, off the async runtime) ─────────
        let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Loading));

        let model_path_clone = model_path.clone();
        let whisper_ctx = tokio::task::spawn_blocking(move || {
            transcribe::load_model(&model_path_clone)
        })
        .await??;

        // Wrap in Arc so the inference closure can own a reference
        let whisper_ctx = Arc::new(whisper_ctx);

        // ── Step 3: Open microphone ──────────────────────────────────────
        let (handle, audio_buffer) = match capture::start_capture() {
            Ok(result) => result,
            Err(e) => {
                let msg = format!("Failed to open microphone: {}", e);
                eprintln!("[operon-voice] {}", msg);
                let _ = tx.send(VoiceEvent::Error(msg));
                let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Idle));
                return Ok(());
            }
        };

        // Store the capture handle so `stop()` can drop it
        {
            let mut ch = self.capture_handle.lock().await;
            *ch = Some(handle);
        }

        // ── Step 4: Listening ────────────────────────────────────────────
        let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Listening));

        // ── Step 5: VAD loop — accumulate audio until silence or stop ────
        let stop_signal = Arc::clone(&self.stop_signal);
        let capture_handle = Arc::clone(&self.capture_handle);

        // Spawn the VAD + transcription pipeline in a background task
        tokio::spawn(async move {
            // VAD configuration: 1.5 seconds of silence to trigger end-of-speech
            let mut silence_detector = vad::SilenceDetector::new(
                vad::VAD_SAMPLE_RATE,  // 16000 Hz
                1.5,                   // 1.5 seconds of silence to stop
            );

            // Poll the audio buffer every 100ms, checking for silence or stop signal
            let poll_interval = tokio::time::Duration::from_millis(100);
            let mut total_samples_seen: usize = 0;

            loop {
                tokio::time::sleep(poll_interval).await;

                // Check if stop was requested by the user
                let should_stop = {
                    let stop = stop_signal.lock().await;
                    *stop
                };

                // Read any new samples from the capture buffer
                let current_samples = {
                    let buf = audio_buffer.lock().unwrap();
                    buf.clone()
                };

                // Feed new samples into the silence detector
                if current_samples.len() > total_samples_seen {
                    let new_samples = &current_samples[total_samples_seen..];
                    silence_detector.feed(new_samples);
                    total_samples_seen = current_samples.len();
                }

                // Stop conditions: user clicked stop OR VAD detected prolonged silence
                // (but only after we have at least 0.5s of audio to avoid empty transcriptions)
                let have_enough_audio = total_samples_seen > (vad::VAD_SAMPLE_RATE as usize / 2);
                let silence_triggered = have_enough_audio && silence_detector.is_silent();

                if should_stop || silence_triggered {
                    break;
                }
            }

            // ── Step 6: Stop capture and run transcription ───────────────
            // Drop the capture handle to close the audio stream
            {
                let mut ch = capture_handle.lock().await;
                *ch = None;
            }

            let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Transcribing));

            // Grab the final audio buffer for transcription
            let final_audio = {
                let buf = audio_buffer.lock().unwrap();
                buf.clone()
            };

            // Only transcribe if we have meaningful audio (> 0.3 seconds)
            if final_audio.len() < (vad::VAD_SAMPLE_RATE as usize / 3) {
                eprintln!(
                    "[operon-voice] Audio too short ({} samples), skipping transcription",
                    final_audio.len()
                );
                let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Idle));
                return;
            }

            // ── Step 7: Whisper inference (blocking, off the async runtime)
            let ctx = Arc::clone(&whisper_ctx);
            let result = tokio::task::spawn_blocking(move || {
                transcribe::transcribe(&ctx, &final_audio)
            })
            .await;

            match result {
                Ok(Ok(text)) => {
                    // Trim whitespace that whisper sometimes prepends/appends
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        let _ = tx.send(VoiceEvent::FinalTranscript(trimmed));
                    }
                }
                Ok(Err(e)) => {
                    let msg = format!("Transcription failed: {}", e);
                    eprintln!("[operon-voice] {}", msg);
                    let _ = tx.send(VoiceEvent::Error(msg));
                }
                Err(e) => {
                    let msg = format!("Transcription task panicked: {}", e);
                    eprintln!("[operon-voice] {}", msg);
                    let _ = tx.send(VoiceEvent::Error(msg));
                }
            }

            // ── Step 8-10: Cleanup — whisper_ctx is dropped here (Arc refcount
            // reaches zero), releasing model RAM. Then signal Idle.
            // The Arc<WhisperContext> goes out of scope here, dropping the model.
            drop(whisper_ctx);

            let _ = tx.send(VoiceEvent::StateChanged(VoiceState::Idle));
        });

        Ok(())
    }

    /// Signal the engine to stop recording.
    ///
    /// This sets the stop flag that the VAD loop checks. The loop will break,
    /// run transcription on whatever audio has been captured, emit the
    /// `FinalTranscript`, and then transition back to Idle.
    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut stop = self.stop_signal.lock().await;
        *stop = true;
        Ok(())
    }
}
