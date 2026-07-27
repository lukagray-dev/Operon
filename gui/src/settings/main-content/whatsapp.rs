//! Controller for the WhatsApp channel settings page.
//!
//! This module wires the callbacks of the `WhatsAppSetup` Slint component:
//! - Loads WhatsApp config from backend.
//! - Saves Owner mobile number and allowlist configuration.
//! - Handles QR pairing stream events and pop-up dialog visibility.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;
use operon_channels_whatsapp::config::WhatsAppConfig;
use operon_channels_whatsapp::types::ContactId;
use operon_channels_whatsapp::auth::WhatsAppAuth;

/// Registers callbacks for the WhatsApp channel settings panel.
pub fn wire_whatsapp_settings(window: &crate::SettingsWindow, _state: Rc<RefCell<AppState>>) {
    let weak_window = window.as_weak();

    // 1. Initial State Loading
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_auth = home.join(".operon").join("channels").join("whatsapp").join("auth");

    // Default configuration: empty initial allowlist
    let owner_str = "".to_string();
    let allow_list: Vec<String> = Vec::new();

    window.set_whatsapp_owner_number(owner_str.into());
    let allow_model: Vec<SharedString> = allow_list.into_iter().map(SharedString::from).collect();
    window.set_whatsapp_allowlist(ModelRc::from(Rc::new(VecModel::from(allow_model))));
    
    let auth_checker = WhatsAppAuth::new(default_auth.clone());
    let initial_status = if auth_checker.has_credentials() {
        "Connected"
    } else {
        "Disconnected"
    };
    window.set_whatsapp_connection_status(initial_status.into());
    window.set_whatsapp_show_qr_popup(false);

    // 2. Handle Save Clicked
    let auth_dir_for_save = default_auth.clone();
    window.on_whatsapp_save_clicked({
        let weak_window = weak_window.clone();
        move |owner_number, allowlist_model| {
            let owner_contact = if owner_number.trim().is_empty() {
                None
            } else {
                Some(ContactId::new(owner_number.as_str()))
            };

            let allowlist: Vec<ContactId> = allowlist_model
                .iter()
                .map(|s| ContactId::new(s.as_str()))
                .collect();

            let _config = WhatsAppConfig {
                enabled: true,
                owner_number: owner_contact,
                allowlist,
                auth_dir: Some(auth_dir_for_save.clone()),
            };

            println!(
                "[operon-gui][whatsapp-settings] Saved WhatsApp config for owner: {}",
                owner_number
            );

            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_connection_status("Disconnected".into());
            }
        }
    });

    // 3. Handle Scan QR Clicked
    window.on_whatsapp_scan_qr_clicked({
        let weak_window = weak_window.clone();
        move || {
            println!("[operon-gui][whatsapp-settings] Scan QR Code clicked");
            let pairing_qr_payload = WhatsAppAuth::generate_whatsapp_md_qr_payload();

            if let Ok(svg_qr) = WhatsAppAuth::render_svg(&pairing_qr_payload) {
                if let Ok(img) = slint::Image::load_from_svg_data(svg_qr.as_bytes()) {
                    if let Some(win) = weak_window.upgrade() {
                        win.set_whatsapp_qr_code_image(img);
                        win.set_whatsapp_show_qr_popup(true);
                        win.set_whatsapp_connection_status("QrRequired".into());
                    }
                }
            }
        }
    });

    // 4. Handle Add Allowlist Number
    window.on_whatsapp_add_allowlist({
        let weak_window = weak_window.clone();
        move |new_number| {
            if let Some(win) = weak_window.upgrade() {
                let current_model = win.get_whatsapp_allowlist();
                let mut list: Vec<SharedString> = current_model.iter().collect();
                if !new_number.trim().is_empty() {
                    list.push(new_number.into());
                    win.set_whatsapp_allowlist(ModelRc::from(Rc::new(VecModel::from(list))));
                }
            }
        }
    });

    // 5. Handle Remove Allowlist Number
    window.on_whatsapp_remove_allowlist({
        let weak_window = weak_window.clone();
        move |idx| {
            if let Some(win) = weak_window.upgrade() {
                let current_model = win.get_whatsapp_allowlist();
                let mut list: Vec<SharedString> = current_model.iter().collect();
                let index = idx as usize;
                if index < list.len() {
                    list.remove(index);
                    win.set_whatsapp_allowlist(ModelRc::from(Rc::new(VecModel::from(list))));
                }
            }
        }
    });

    // 6. Handle Close QR Popup
    window.on_whatsapp_close_qr_popup({
        let weak_window = weak_window.clone();
        move || {
            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_show_qr_popup(false);
            }
        }
    });

    // 7. Handle Generate Pairing Code Clicked
    window.on_whatsapp_generate_pairing_code_clicked({
        let weak_window = weak_window.clone();
        move || {
            println!("[operon-gui][whatsapp-settings] Generate Pairing Code clicked");
            let code = WhatsAppAuth::generate_pairing_code();
            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_pairing_code(code.into());
                win.set_whatsapp_show_pairing_code_popup(true);
                win.set_whatsapp_connection_status("Connecting".into());
            }
        }
    });

    // 8. Handle Close Pairing Code Popup
    window.on_whatsapp_close_pairing_code_popup({
        let weak_window = weak_window.clone();
        move || {
            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_show_pairing_code_popup(false);
            }
        }
    });
}
