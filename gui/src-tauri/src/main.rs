// Operon GUI - Tauri Application Entry Point
//
// This file provides the main entry point for the Operon graphical interface.
// It uses Tauri to create a native desktop application with a web-based UI.
//
// The application integrates with the Operon backend to provide a visual
// interface for interacting with the AI agent.

// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        // Register plugins
        .plugin(tauri_plugin_opener::init())
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
