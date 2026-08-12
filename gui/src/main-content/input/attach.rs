//! Attachment button controller.
//!
//! Handles native file selection picker dialogs to attach files/images to the prompt context.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::media::{self, PendingAttachment};
use crate::state::AppState;

/// Updates the Slint UI model for pending attachment chips.
pub fn update_attachment_chips(window: &crate::OperonWindow, state: &AppState) {
    let chips: Vec<crate::AttachmentChip> = state
        .pending_attachments()
        .iter()
        .map(|att| match att {
            PendingAttachment::Image {
                cached_path,
                display_name,
                ..
            } => {
                let thumbnail = slint::Image::load_from_path(cached_path).unwrap_or_default();
                crate::AttachmentChip {
                    display_name: display_name.clone().into(),
                    is_image: true,
                    thumbnail,
                }
            }
            PendingAttachment::File { display_name, .. } => crate::AttachmentChip {
                display_name: display_name.clone().into(),
                is_image: false,
                thumbnail: slint::Image::default(),
            },
        })
        .collect();

    window.set_pending_attachments(slint::ModelRc::from(Rc::new(slint::VecModel::from(chips))));
}

/// Register attachment button click callback.
pub fn wire_attach(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let state_clone = Rc::clone(&state);

    window.on_attach_clicked(move || {
        println!("[operon-gui][input] Attachment button clicked.");

        // Disable picker if active provider is Cohere
        if let Ok(app_config) = operon_rs::load() {
            if app_config.provider.provider == operon_rs::providers::Provider::Cohere {
                println!(
                    "[operon-gui][input] Attachment button clicked but Cohere provider is active; attachments are disabled."
                );
                return;
            }
        }

        let picked_file = rfd::FileDialog::new()
            .set_title("Attach File to Chat Context")
            .pick_file();

        if let Some(path_buf) = picked_file {
            let display_name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("attached_file")
                .to_string();

            if media::is_image_mime(&path_buf) {
                match media::cache_image(&path_buf) {
                    Ok(cached_path) => match media::encode_base64(&cached_path) {
                        Ok((media_type, base64_data)) => {
                            let attachment = PendingAttachment::Image {
                                cached_path,
                                media_type,
                                base64_data,
                                display_name,
                            };
                            state_clone.borrow_mut().add_attachment(attachment);
                        }
                        Err(e) => {
                            eprintln!("[operon-gui][attach] Failed to encode base64 image: {e}");
                        }
                    },
                    Err(e) => {
                        eprintln!("[operon-gui][attach] Failed to cache image: {e}");
                    }
                }
            } else {
                let attachment = PendingAttachment::File {
                    path: path_buf,
                    display_name,
                };
                state_clone.borrow_mut().add_attachment(attachment);
            }

            if let Some(win) = window_weak.upgrade() {
                update_attachment_chips(&win, &state_clone.borrow());
            }
        }
    });
}

/// Register attachment removal callback.
pub fn wire_attachment_removed(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let state_clone = Rc::clone(&state);

    window.on_attachment_removed(move |idx| {
        let idx = idx as usize;
        state_clone.borrow_mut().remove_attachment(idx);

        if let Some(win) = window_weak.upgrade() {
            update_attachment_chips(&win, &state_clone.borrow());
        }
    });
}
