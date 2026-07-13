//! Voice transcription record button controller.
//!
//! Handles microphone toggling and voice transcription services.

use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;

use crate::state::AppState;

/// Register voice recording event callbacks.
pub fn wire_voice(
    window: &crate::OperonWindow,
    _state: Rc<RefCell<AppState>>,
) {
    let window_weak = window.as_weak();

    window.on_voice_clicked({
        let window_weak = window_weak.clone();
        move || {
            if let Some(win) = window_weak.upgrade() {
                let recording = !win.get_is_recording();
                println!("[operon-gui][input] Voice button clicked. Recording state: {}", recording);
                win.set_is_recording(recording);
            }
        }
    });

    window.on_voice_recording_started(move || {
        println!("[operon-gui][input] Voice recording started.");
    });

    window.on_voice_recording_stopped(move || {
        println!("[operon-gui][input] Voice recording stopped.");
    });
}
