// Operon GUI - Tauri Application Entry Point
//
// This file provides the main entry point for the Operon graphical interface.
// It uses Tauri to create a native desktop application with a web-based UI.
//
// The application integrates with the Operon backend to provide a visual
// interface for interacting with the AI agent.

// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use tauri::Manager;

mod commands;

fn main() {
    // Initialize shared application state
    let app_state = Arc::new(Mutex::new(commands::AppState::default()));

    // Load initial configuration on startup
    if let Ok(config) = operon_rs::load() {
        if let Ok(mut state) = app_state.lock() {
            state.active_config = Some(config.provider);
        }
    }

    tauri::Builder::default()
        // Register plugins
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Register shared state
        .manage(app_state)
        // Register IPC command handlers
        .invoke_handler(tauri::generate_handler![
            commands::markdown_commands::render_markdown,
            commands::model_commands::get_model_providers,
            commands::model_commands::get_model_provider_setup,
            commands::model_commands::discover_models,
            commands::model_commands::save_provider_setup,
            commands::model_commands::get_active_provider,
            commands::permission_commands::get_allowed_directories,
            commands::permission_commands::add_allowed_directory,
            commands::permission_commands::remove_allowed_directory,
            commands::permission_commands::get_permission_rows,
            commands::permission_commands::update_permission_mode,
            commands::session_commands::list_sessions,
            commands::session_commands::get_session_history,
            commands::session_commands::send_message,
            commands::session_commands::cancel_session,
            commands::session_commands::approve_tool_call,
            commands::session_commands::deny_tool_call,
            commands::session_commands::open_project_folder,
            commands::session_commands::get_default_workspace,
        ])
        // Setup window and app configuration
        .setup(|app| {
            // Get the main window
            if let Some(window) = app.get_webview_window("main") {
                // Optional: Set window properties or event listeners here
                #[cfg(debug_assertions)]
                {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        // Run the application
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
