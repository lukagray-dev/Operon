//! Operon Tauri GUI backend library.

#[path = "left-sidebar/mod.rs"]
pub mod left_sidebar;
#[path = "main-content/mod.rs"]
pub mod main_content;
pub mod shared;
pub mod titlebar;

use shared::AppState;
use tauri::Manager;

/// Applies Windows-specific DWM attributes for sharp corners and no light border outline.
fn apply_window_dwm_styling(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWCP_DONOTROUND,
        };

        if let Ok(hwnd) = window.hwnd() {
            let raw_hwnd = hwnd.0 as isize as *mut std::ffi::c_void;

            // 1. Force sharp rectangular corners (DWMWCP_DONOTROUND = 1)
            let corner_pref = DWMWCP_DONOTROUND;
            unsafe {
                DwmSetWindowAttribute(
                    raw_hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &corner_pref as *const _ as *const _,
                    std::mem::size_of_val(&corner_pref) as u32,
                );

                // 2. Remove white outline by matching border color to titlebar background (#191919)
                let border_color: u32 = 0x00191919;
                DwmSetWindowAttribute(
                    raw_hwnd,
                    DWMWA_BORDER_COLOR as u32,
                    &border_color as *const _ as *const _,
                    std::mem::size_of_val(&border_color) as u32,
                );
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(main_window) = app.get_webview_window("main") {
                apply_window_dwm_styling(&main_window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Window actions
            titlebar::minimize_window,
            titlebar::toggle_maximize_window,
            titlebar::is_window_maximized,
            titlebar::close_window,
            titlebar::start_dragging,
            titlebar::toggle_sidebar,
            titlebar::get_sidebar_state,
            // Menu actions
            titlebar::open_external_url,
            titlebar::open_documentation,
            titlebar::open_report_bug,
            titlebar::open_follow_creator,
            titlebar::open_repository,
            titlebar::exit_application,
            // Left Sidebar actions
            left_sidebar::query_sidebar_data,
            left_sidebar::delete_session,
            left_sidebar::delete_project,
            left_sidebar::open_project_picker,
            left_sidebar::create_new_session,
            left_sidebar::rename_session,
            left_sidebar::fork_session,
            left_sidebar::move_session,
            left_sidebar::query_whatsapp_contacts,
            left_sidebar::query_telegram_contacts,
            // Main Content Input actions
            main_content::input::get_available_models,
            main_content::input::select_model,
            main_content::input::toggle_auto_approve,
            main_content::input::pick_attachments_dialog,
            main_content::input::get_context_window_info,
            // Main Content Topbar actions
            main_content::topbar::get_git_diff_stats,
            main_content::topbar::get_topbar_info,
            // Main Content Messages actions
            main_content::messages::load_session_messages,
            main_content::messages::submit_prompt,
            main_content::messages::cancel_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
