//! Voice transcription button controller.
//!
//! This module wires the voice button's `clicked` callback to the full
//! voice-to-text pipeline:
//!
//!   Idle → (click) → Loading → Listening → (click or VAD silence) →
//!   Transcribing → text inserted into input → Idle
//!
//! The heavy lifting happens in `operon-voice` (via the `operon-rs` facade).
//! This file only handles GUI event plumbing: spawning async tasks, forwarding
//! `VoiceEvent`s to Slint via `invoke_from_event_loop`, and inserting the
//! final transcript into the input text field.
//!
//! ## Threading Model
//!
//! - The `on_voice_clicked` callback runs on the Slint UI thread.
//! - `VoiceEngine::start()` / `stop()` are async and run on the tokio runtime.
//! - Slint property mutations happen only via `slint::invoke_from_event_loop`
//!   from the tokio task, never directly from a background thread.

use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Global VoiceEngine handle
// ─────────────────────────────────────────────────────────────────────────────

/// Global handle to the VoiceEngine, shared between the UI click callback
/// and the background tokio task.
///
/// We use a `Mutex<Option<...>>` (same pattern as `executor::ACTIVE_CMD_TX`)
/// because the engine needs to be accessed from both the Slint UI thread
/// (for click handling) and tokio tasks (for start/stop).
///
/// The engine is created once during `wire_voice()` and persists for the
/// app's lifetime. The whisper model inside it is loaded/dropped per session.
static VOICE_ENGINE: Mutex<Option<std::sync::Arc<operon_rs::VoiceEngine>>> = Mutex::new(None);

// ─────────────────────────────────────────────────────────────────────────────
// Public wiring
// ─────────────────────────────────────────────────────────────────────────────

/// Register voice recording event callbacks on the Slint window.
///
/// This sets up:
/// 1. A `VoiceEngine` pointing at `~/.operon/models/ggml-tiny.en.bin`.
/// 2. The `on_voice_clicked` callback that drives the state machine.
/// 3. Stub callbacks for `recording-started` and `recording-stopped`.
pub fn wire_voice(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    // ── Resolve model path using the existing OperonPaths helper ──────────
    // This reuses the same path resolution as the rest of Operon — no
    // hardcoded dirs::home_dir() call needed.
    let model_path = match operon_rs::config::OperonPaths::resolve() {
        Ok(paths) => paths.config_dir.join("models").join("ggml-tiny.en.bin"),
        Err(e) => {
            eprintln!(
                "[operon-gui][voice] Failed to resolve Operon paths: {}. \
                 Voice transcription will be unavailable.",
                e
            );
            // Use a fallback path that will fail gracefully at start() time
            // with a clear error message about the missing model file
            std::path::PathBuf::from("ggml-tiny.en.bin")
        }
    };

    eprintln!(
        "[operon-gui][voice] Model path: {}",
        model_path.display()
    );

    // ── Create and store the VoiceEngine ─────────────────────────────────
    let engine = std::sync::Arc::new(operon_rs::VoiceEngine::new(model_path));
    {
        let mut guard = VOICE_ENGINE.lock().unwrap();
        *guard = Some(std::sync::Arc::clone(&engine));
    }

    // ── Wire the click callback ──────────────────────────────────────────
    let window_weak = window.as_weak();

    window.on_voice_clicked(move || {
        let Some(win) = window_weak.upgrade() else {
            return;
        };

        // Read current voice state from the Slint property
        let current_state = win.get_voice_state();

        match current_state {
            // Idle (0) → Start recording
            0 => {
                eprintln!("[operon-gui][voice] Starting voice capture...");

                // Grab the engine handle
                let engine = {
                    let guard = VOICE_ENGINE.lock().unwrap();
                    guard.clone()
                };

                let Some(engine) = engine else {
                    eprintln!("[operon-gui][voice] VoiceEngine not initialized");
                    return;
                };

                // Set state to Loading immediately for responsive UI feedback
                win.set_voice_state(1);

                // Create a channel for VoiceEvents
                let (tx, mut rx) =
                    tokio::sync::mpsc::unbounded_channel::<operon_rs::VoiceEvent>();

                let win_weak = win.as_weak();

                // Spawn the engine start task
                tokio::spawn(async move {
                    // Start the engine — this kicks off model loading, capture, etc.
                    if let Err(e) = engine.start(tx).await {
                        eprintln!("[operon-gui][voice] Engine start failed: {}", e);
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win_weak.upgrade() {
                                w.set_voice_state(0);
                            }
                        });
                        return;
                    }

                    // Event forwarding loop: receive VoiceEvents and update Slint
                    while let Some(event) = rx.recv().await {
                        let win_weak_clone = win_weak.clone();
                        match event {
                            operon_rs::VoiceEvent::StateChanged(state) => {
                                let state_int = state as i32;
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win_weak_clone.upgrade() {
                                        w.set_voice_state(state_int);
                                    }
                                });
                            }
                            operon_rs::VoiceEvent::FinalTranscript(text) => {
                                // Insert the transcribed text into the input field
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win_weak_clone.upgrade() {
                                        // Append to existing text with a space separator
                                        let existing = w.get_input_text().to_string();
                                        let new_text = if existing.is_empty() {
                                            text
                                        } else {
                                            format!("{} {}", existing, text)
                                        };
                                        w.set_input_text(new_text.into());
                                        eprintln!(
                                            "[operon-gui][voice] Transcript inserted into input"
                                        );
                                    }
                                });
                            }
                            operon_rs::VoiceEvent::PartialTranscript(_text) => {
                                // Partial transcripts are not used yet — whisper.cpp
                                // doesn't support true incremental streaming.
                                // This variant exists for future compatibility.
                            }
                            operon_rs::VoiceEvent::Error(msg) => {
                                eprintln!("[operon-gui][voice] Error: {}", msg);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win_weak_clone.upgrade() {
                                        w.set_voice_state(0);
                                    }
                                });
                            }
                        }
                    }
                });
            }

            // Listening (2) → Stop recording (user clicked again)
            2 => {
                eprintln!("[operon-gui][voice] Stopping voice capture...");

                let engine = {
                    let guard = VOICE_ENGINE.lock().unwrap();
                    guard.clone()
                };

                if let Some(engine) = engine {
                    // Spawn the stop task — this signals the VAD loop to break,
                    // which triggers transcription and then returns to Idle
                    tokio::spawn(async move {
                        if let Err(e) = engine.stop().await {
                            eprintln!("[operon-gui][voice] Engine stop failed: {}", e);
                        }
                    });
                }
            }

            // Loading (1) or Transcribing (3) → Ignore clicks during these
            // transient states. The user must wait for the operation to complete.
            _ => {
                eprintln!(
                    "[operon-gui][voice] Click ignored — engine busy (state={})",
                    current_state
                );
            }
        }
    });

    // ── Backward-compatible stub callbacks ───────────────────────────────
    // These are still declared in the Slint component for compatibility with
    // input.slint's callback wiring. They just log — the real work happens
    // in on_voice_clicked above.
    window.on_voice_recording_started(|| {
        eprintln!("[operon-gui][voice] Recording started (callback)");
    });

    window.on_voice_recording_stopped(|| {
        eprintln!("[operon-gui][voice] Recording stopped (callback)");
    });
}
