//! Operon Tauri GUI backend library.

use tauri::Manager;

/// Example command to demonstrate invocation from TypeScript.
#[tauri::command]
async fn send_prompt(prompt: String) -> Result<String, String> {
    // In full implementation, this forwards the prompt to operon-rs session runner.
    Ok(format!("Operon Backend received: \"{}\"", prompt))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Setup hooks if needed
            let _window = app.get_webview_window("main");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![send_prompt])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
