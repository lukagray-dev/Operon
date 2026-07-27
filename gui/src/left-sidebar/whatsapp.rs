//! WhatsApp sidebar controller orchestration.
//!
//! Queries WhatsApp contacts and session history from `~/.operon/channels/whatsapp/workspace/`
//! and `~/.operon/sessions/whatsapp/`, builds Slint models, and wires click handlers.

use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::state::AppState;
use crate::{SidebarConversation, SidebarProject};

/// Query WhatsApp contacts and session JSON files from disk and construct Slint SidebarProject DTOs.
pub fn load_whatsapp_sidebar_data() -> Vec<SidebarProject> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_sessions = home.join(".operon").join("sessions").join("whatsapp");

    if !base_sessions.exists() {
        return Vec::new();
    }

    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&base_sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let contact_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown Contact")
                    .to_string();

                let mut conversations = Vec::new();

                if let Ok(sess_entries) = std::fs::read_dir(&path) {
                    for sess_entry in sess_entries.flatten() {
                        let sess_path = sess_entry.path();
                        if sess_path.is_file()
                            && sess_path.extension().and_then(|e| e.to_str()) == Some("json")
                        {
                            let session_id = sess_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();

                            let title = format!("Session {}", &session_id[..session_id.len().min(8)]);

                            conversations.push(SidebarConversation {
                                id: session_id.into(),
                                title: title.into(),
                            });
                        }
                    }
                }

                if !conversations.is_empty() {
                    let conv_model: Vec<SidebarConversation> = conversations;
                    projects.push(SidebarProject {
                        name: contact_name.into(),
                        workspace: path.to_string_lossy().to_string().into(),
                        conversations: ModelRc::from(Rc::new(VecModel::from(conv_model))),
                    });
                }
            }
        }
    }

    projects
}

/// Register WhatsApp sidebar setup and session selection actions.
pub fn wire_whatsapp(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();

    // Populate initial WhatsApp contacts in sidebar
    let whatsapp_data = load_whatsapp_sidebar_data();
    window.set_sidebar_whatsapp_contacts(ModelRc::from(Rc::new(VecModel::from(whatsapp_data))));

    // Callback: WhatsApp session clicked
    window.on_sidebar_whatsapp_session_clicked({
        let window_weak = window_weak.clone();
        let app_state = Rc::clone(&state);
        move |session_id: slint::SharedString, contact_number: slint::SharedString| {
            if let Some(win) = window_weak.upgrade() {
                println!(
                    "[operon-gui][whatsapp-sidebar] Clicked session {} for contact {}",
                    session_id, contact_number
                );

                // Set read-only posture for WhatsApp sessions
                win.set_is_read_only_session(true);

                // Load chat session messages
                crate::left_sidebar::load_chat_session(&win, session_id.as_str(), None, &app_state);
            }
        }
    });
}
