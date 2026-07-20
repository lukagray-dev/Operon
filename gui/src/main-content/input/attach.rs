//! Attachment button controller.
//!
//! Handles native file selection picker dialogs to attach files/images to the prompt context.

use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Register attachment button click callback.
pub fn wire_attach(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    window.on_attach_clicked(move || {
        println!("[operon-gui][input] Attachment button clicked.");

        let picked_file = rfd::FileDialog::new()
            .set_title("Attach File to Chat Context")
            .pick_file();

        if let Some(path_buf) = picked_file {
            let path_str = path_buf.to_string_lossy().to_string();
            println!("[operon-gui][input] User attached file: {}", path_str);

            // In a future integration, we can display the attachment list in the UI
            // and pass the file content or reference along with the message payload
        }
    });
}
