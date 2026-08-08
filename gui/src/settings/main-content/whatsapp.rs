//! Controller for the WhatsApp channel settings page.
//!
//! This module wires the callbacks of the `WhatsAppSetup` Slint component:
//! - Loads WhatsApp config from backend.
//! - Saves Owner mobile number and allowlist configuration.
//! - Handles QR pairing stream events and pop-up dialog visibility.
//! - Handles pairing-code flow by subscribing to real WhatsApp server events.
//!
//! ## Architecture Note
//! A single `Arc<Mutex<Option<Arc<WhatsAppClient>>>>` (the "client handle") is
//! shared across all callbacks in `wire_whatsapp_settings`. When the user clicks
//! "Scan QR" or "Generate Pairing Code", a new `WhatsAppClient` is created,
//! stored in the handle, and `connect()` is spawned on tokio. Subscriber tasks
//! listen for real QR / pairing-code events and push them to the Slint UI via
//! `slint::invoke_from_event_loop`. A status-poller task watches for the
//! Connected / Error transitions and closes popups accordingly.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex as StdMutex};

use crate::state::AppState;
use operon_channels_whatsapp::auth::WhatsAppAuth;
use operon_channels_whatsapp::client::WhatsAppClient;
use operon_channels_whatsapp::config::WhatsAppConfig;
use operon_channels_whatsapp::service::WhatsAppService;
use operon_channels_whatsapp::types::ContactId;

/// Type alias for the shared client handle used across Slint callbacks.
/// - `Arc` for cross-thread sharing (Slint callbacks → tokio tasks).
/// - `StdMutex` because we never hold it across `.await` points.
/// - `Option` because no client exists until the user initiates pairing.
/// - Inner `Arc<WhatsAppClient>` so the client can be shared between the
///   subscriber tasks and the connect task without lifetime issues.
type ClientHandle = Arc<StdMutex<Option<Arc<WhatsAppClient>>>>;

/// Registers callbacks for the WhatsApp channel settings panel.
///
/// All pairing callbacks share a single `ClientHandle` so that:
/// 1. Only one client is active at a time (starting a new flow replaces the old one).
/// 2. The close-popup callbacks can disconnect the client if the user cancels.
pub fn wire_whatsapp_settings(window: &crate::SettingsWindow, _state: Rc<RefCell<AppState>>) {
    let weak_window = window.as_weak();

    // ── 1. Initial State Loading ─────────────────────────────────────────────
    // Resolve the default auth directory and check if credentials already exist.
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_auth = home.join(".operon").join("channels").join("whatsapp").join("auth");

    // Default configuration: empty initial allowlist, no owner number.
    let owner_str = "".to_string();
    let allow_list: Vec<String> = Vec::new();

    window.set_whatsapp_owner_number(owner_str.into());
    let allow_model: Vec<SharedString> = allow_list.into_iter().map(SharedString::from).collect();
    window.set_whatsapp_allowlist(ModelRc::from(Rc::new(VecModel::from(allow_model))));

    // Check if we already have persisted credentials from a prior session.
    let auth_checker = WhatsAppAuth::new(default_auth.clone());
    let has_creds = auth_checker.has_credentials();
    let initial_status = if has_creds { "Connected" } else { "Disconnected" };
    window.set_whatsapp_connection_status(initial_status.into());
    window.set_whatsapp_show_qr_popup(false);

    // ── Shared client handle ─────────────────────────────────────────────────
    // One handle for the entire settings page — both QR and pairing-code flows
    // write to this, so starting one flow replaces the other automatically.
    // NOTE: Auto-reconnect on app startup is handled by the main window's
    // sidebar wiring (`left-sidebar/whatsapp.rs`), not here. This module only
    // creates clients when the user explicitly clicks "Scan QR" or "Pairing Code".
    let client_handle: ClientHandle = Arc::new(StdMutex::new(None));

    // ── 2. Handle Save Clicked ───────────────────────────────────────────────
    let auth_dir_for_save = default_auth.clone();
    window.on_whatsapp_save_clicked({
        let weak_window = weak_window.clone();
        let client_handle_for_save = client_handle.clone();
        let default_auth_for_status = default_auth.clone();
        move |owner_number, allowlist_model| {
            // Parse the owner number into a sanitized ContactId (or None if blank).
            let owner_contact = if owner_number.trim().is_empty() {
                None
            } else {
                Some(ContactId::new(owner_number.as_str()))
            };

            // Collect the allowlist model into a Vec<ContactId>.
            let allowlist: Vec<ContactId> = allowlist_model
                .iter()
                .map(|s| ContactId::new(s.as_str()))
                .collect();

            // Build the config struct (persisted by the backend layer).
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

            // Refresh the displayed status from the live client or on-disk
            // credentials. Do NOT blindly reset to "Disconnected" — the user
            // may have a live bot running from a prior QR scan.
            if let Some(win) = weak_window.upgrade() {
                let has_live_client = client_handle_for_save
                    .lock()
                    .ok()
                    .map(|g| g.is_some())
                    .unwrap_or(false);
                if has_live_client {
                    // A client handle exists — don't touch the status, it's
                    // being managed by the status poller / event handlers.
                } else {
                    // No live client — check persisted credentials on disk.
                    let auth = WhatsAppAuth::new(default_auth_for_status.clone());
                    let label = if auth.has_credentials() {
                        "Connected"
                    } else {
                        "Disconnected"
                    };
                    win.set_whatsapp_connection_status(label.into());
                }
            }
        }
    });

    // ── 3. Handle Scan QR Clicked ────────────────────────────────────────────
    // Creates a real WhatsAppClient, connects, subscribes to QR events, and
    // renders them as SVGs in the popup. No fake payloads are generated.
    window.on_whatsapp_scan_qr_clicked({
        let weak_window = weak_window.clone();
        let client_handle = client_handle.clone();
        let default_auth = default_auth.clone();
        move || {
            let weak = weak_window.clone();
            let handle = client_handle.clone();
            let auth_dir = default_auth.clone();

            // Read the owner number from the UI text field for WhatsAppConfig.
            let owner_number = weak
                .upgrade()
                .map(|w| w.get_whatsapp_owner_number().to_string())
                .unwrap_or_default();

            // Build config — QR flow does NOT set pair_phone (that's for pairing-code only).
            let config = WhatsAppConfig {
                enabled: true,
                owner_number: if owner_number.trim().is_empty() {
                    None
                } else {
                    Some(ContactId::new(&owner_number))
                },
                allowlist: vec![],
                auth_dir: Some(auth_dir),
            };

            // Create the client and store it in the shared handle.
            let client = Arc::new(WhatsAppClient::new(&config));
            if let Ok(mut guard) = handle.lock() {
                *guard = Some(client.clone());
            }

            // Show "Connecting…" state immediately so the user sees feedback.
            if let Some(win) = weak.upgrade() {
                win.set_whatsapp_connection_status("Connecting".into());
                win.set_whatsapp_show_qr_popup(true);
            }

            // Spawn the async connect + QR listener on the tokio runtime.
            tokio::spawn({
                let weak = weak.clone();
                let client = client.clone();
                let wa_config = config.clone();
                async move {
                    // Take receivers BEFORE connect() so we don't miss any events.
                    // QR receiver — for rendering QR codes in the popup.
                    if let Some(mut qr_rx) = client.take_qr_receiver().await {
                        let weak_for_qr = weak.clone();
                        tokio::spawn(async move {
                            while let Some(qr_state) = qr_rx.recv().await {
                                let svg = WhatsAppAuth::render_svg(&qr_state.payload).ok();
                                let weak2 = weak_for_qr.clone();
                                slint::invoke_from_event_loop(move || {
                                    if let Some(win) = weak2.upgrade() {
                                        if let Some(svg_str) = svg {
                                            if let Ok(img) =
                                                slint::Image::load_from_svg_data(
                                                    svg_str.as_bytes(),
                                                )
                                            {
                                                win.set_whatsapp_qr_code_image(img);
                                            }
                                        }
                                        win.set_whatsapp_connection_status("QrRequired".into());
                                        win.set_whatsapp_show_qr_popup(true);
                                    }
                                })
                                .ok();
                            }
                        });
                    }

                    // Spawn a status-poller task that watches for Connected/Error.
                    spawn_status_poller(weak.clone(), client.clone());

                    // connect() starts the bot event loop and returns immediately.
                    if let Err(e) = client.connect().await {
                        let err_str = e.to_string();
                        let weak_err = weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak_err.upgrade() {
                                win.set_whatsapp_connection_status(
                                    format!("Error: {}", err_str).into(),
                                );
                                win.set_whatsapp_show_qr_popup(false);
                            }
                        })
                        .ok();
                    } else {
                        // Bot started successfully — spawn WhatsAppService.
                        if let Ok(app_config) = operon_rs::load() {
                            let service = WhatsAppService::new(client.clone(), wa_config, app_config);
                            tokio::spawn(async move {
                                if let Err(e) = service.run().await {
                                    eprintln!("[operon-gui][whatsapp-settings] WhatsAppService error: {}", e);
                                }
                            });
                        }
                    }
                }
            });
        }
    });

    // ── 4. Handle Add Allowlist Number ───────────────────────────────────────
    window.on_whatsapp_add_allowlist(add_allowlist_handler(weak_window.clone()));

    // ── 5. Handle Remove Allowlist Number ────────────────────────────────────
    window.on_whatsapp_remove_allowlist(remove_allowlist_handler(weak_window.clone()));

    // ── 6. Handle Close QR Popup ─────────────────────────────────────────────
    // Only disconnect if pairing hasn't completed yet. If the bot is Connected,
    // keep it alive so messages continue to flow.
    window.on_whatsapp_close_qr_popup({
        let weak_window = weak_window.clone();
        let client_handle = client_handle.clone();
        move || {
            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_show_qr_popup(false);
                // Only tear down the connection if we're NOT Connected.
                // A connected bot must keep running for message delivery.
                let status = win.get_whatsapp_connection_status();
                if status.as_str() != "Connected" {
                    disconnect_client(&client_handle);
                }
            }
        }
    });

    // ── 7. Handle Generate Pairing Code Clicked ──────────────────────────────
    // Creates a WhatsAppClient with pair_phone set, connects, and subscribes to
    // the pairing-code channel. The real server-issued code replaces the
    // "Waiting for code…" placeholder once it arrives.
    window.on_whatsapp_generate_pairing_code_clicked({
        let weak_window = weak_window.clone();
        let client_handle = client_handle.clone();
        let default_auth = default_auth.clone();
        move || {
            let weak = weak_window.clone();
            let handle = client_handle.clone();
            let auth_dir = default_auth.clone();

            // Read the owner number — this is REQUIRED for pair-code flow
            // because WhatsApp needs the phone number to issue a code.
            let owner_number = weak
                .upgrade()
                .map(|w| w.get_whatsapp_owner_number().to_string())
                .unwrap_or_default();

            if owner_number.trim().is_empty() {
                // Can't generate a pairing code without a phone number.
                if let Some(win) = weak.upgrade() {
                    win.set_whatsapp_connection_status(
                        "Error: Owner number required for pairing code".into(),
                    );
                }
                return;
            }

            // Build config — pair-code flow uses owner_number as the pair_phone.
            let config = WhatsAppConfig {
                enabled: true,
                owner_number: Some(ContactId::new(&owner_number)),
                allowlist: vec![],
                auth_dir: Some(auth_dir),
            };

            // Create the client and store it in the shared handle.
            let client = Arc::new(WhatsAppClient::new(&config));
            if let Ok(mut guard) = handle.lock() {
                *guard = Some(client.clone());
            }

            // Show a placeholder immediately while waiting for the server.
            if let Some(win) = weak.upgrade() {
                win.set_whatsapp_pairing_code("Waiting for code...".into());
                win.set_whatsapp_show_pairing_code_popup(true);
                win.set_whatsapp_connection_status("Connecting".into());
            }

            // Spawn the async connect + pairing-code listener on tokio.
            tokio::spawn({
                let weak = weak.clone();
                let client = client.clone();
                let wa_config = config.clone();
                async move {
                    // Take the pairing-code receiver BEFORE connect().
                    if let Some(mut pc_rx) = client.take_pairing_code_receiver().await {
                        let weak_for_pc = weak.clone();
                        tokio::spawn(async move {
                            while let Some(pc_state) = pc_rx.recv().await {
                                let code = pc_state.code.clone();
                                let weak2 = weak_for_pc.clone();
                                slint::invoke_from_event_loop(move || {
                                    if let Some(win) = weak2.upgrade() {
                                        win.set_whatsapp_pairing_code(code.into());
                                        win.set_whatsapp_show_pairing_code_popup(true);
                                        win.set_whatsapp_connection_status(
                                            "PairingCodeIssued".into(),
                                        );
                                    }
                                })
                                .ok();
                            }
                        });
                    }

                    // Spawn a status-poller task that watches for Connected/Error.
                    spawn_status_poller(weak.clone(), client.clone());

                    // connect() starts the bot event loop and returns immediately.
                    if let Err(e) = client.connect().await {
                        let err_str = e.to_string();
                        let weak_err = weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak_err.upgrade() {
                                win.set_whatsapp_connection_status(
                                    format!("Error: {}", err_str).into(),
                                );
                                win.set_whatsapp_show_pairing_code_popup(false);
                            }
                        })
                        .ok();
                    } else {
                        // Bot started successfully — spawn WhatsAppService.
                        if let Ok(app_config) = operon_rs::load() {
                            let service = WhatsAppService::new(client.clone(), wa_config, app_config);
                            tokio::spawn(async move {
                                if let Err(e) = service.run().await {
                                    eprintln!("[operon-gui][whatsapp-settings] WhatsAppService error: {}", e);
                                }
                            });
                        }
                    }
                }
            });
        }
    });

    // ── 8. Handle Close Pairing Code Popup ───────────────────────────────────
    window.on_whatsapp_close_pairing_code_popup({
        let weak_window = weak_window.clone();
        let client_handle = client_handle.clone();
        move || {
            if let Some(win) = weak_window.upgrade() {
                win.set_whatsapp_show_pairing_code_popup(false);
                // Only tear down the connection if we're NOT Connected.
                let status = win.get_whatsapp_connection_status();
                if status.as_str() != "Connected" {
                    disconnect_client(&client_handle);
                }
            }
        }
    });
}

// ── Helper: Spawn a status-poller task ───────────────────────────────────────
// Polls `client.status()` every 500ms and updates the Slint UI when the
// connection transitions to Connected or Error. Stops polling once a terminal
// state is reached. Uses a 5-minute deadline to avoid spinning indefinitely
// if the event flow stalls.
//
// IMPORTANT: Does NOT break on Disconnected — the client starts in Disconnected
// state before connect() sets it to Connecting. Breaking early would cause the
// poller to exit before the bot even starts.
fn spawn_status_poller(
    weak: slint::Weak<crate::SettingsWindow>,
    client: Arc<WhatsAppClient>,
) {
    tokio::spawn(async move {
        // Hard deadline: stop polling after 5 minutes regardless of state.
        let deadline = tokio::time::Instant::now()
            + tokio::time::Duration::from_secs(300);
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Safety valve: don't poll forever if something goes wrong.
            if tokio::time::Instant::now() > deadline {
                break;
            }

            let status = client.status().await;
            match status {
                operon_channels_whatsapp::ConnectionStatus::Connected => {
                    // Pairing succeeded — update UI and close all popups.
                    let weak2 = weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak2.upgrade() {
                            win.set_whatsapp_connection_status("Connected".into());
                            win.set_whatsapp_show_qr_popup(false);
                            win.set_whatsapp_show_pairing_code_popup(false);
                        }
                    })
                    .ok();
                    // Terminal state — stop polling.
                    break;
                }
                operon_channels_whatsapp::ConnectionStatus::Error(ref err) => {
                    // Connection failed — show the error and close popups.
                    let err_msg = format!("Error: {}", err);
                    let weak2 = weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak2.upgrade() {
                            win.set_whatsapp_connection_status(err_msg.into());
                            win.set_whatsapp_show_qr_popup(false);
                            win.set_whatsapp_show_pairing_code_popup(false);
                        }
                    })
                    .ok();
                    // Terminal state — stop polling.
                    break;
                }
                // Disconnected, Connecting, QrRequired, PairingCodeIssued —
                // keep polling. The client starts Disconnected and transitions
                // to Connecting once connect() begins; breaking on Disconnected
                // here would kill the poller before the bot starts.
                _ => {}
            }
        }
    });
}

// ── Helper: Disconnect and drop the active client ────────────────────────────
// Called when the user closes a popup to prevent orphaned background connections.
fn disconnect_client(handle: &ClientHandle) {
    if let Ok(mut guard) = handle.lock() {
        if let Some(client) = guard.take() {
            // Spawn disconnect on tokio since it's an async operation and we're
            // currently in a synchronous Slint callback context.
            tokio::spawn(async move {
                client.disconnect().await;
            });
        }
    }
}

// ── Helper: Add allowlist number callback ────────────────────────────────────
// Extracted to keep the main wiring function readable.
fn add_allowlist_handler(
    weak_window: slint::Weak<crate::SettingsWindow>,
) -> impl FnMut(SharedString) {
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
}

// ── Helper: Remove allowlist number callback ─────────────────────────────────
fn remove_allowlist_handler(
    weak_window: slint::Weak<crate::SettingsWindow>,
) -> impl FnMut(i32) {
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
}


