//! Operon Tauri GUI backend library.

#[path = "left-sidebar/mod.rs"]
pub mod left_sidebar;
#[path = "main-content/mod.rs"]
pub mod main_content;
#[path = "right-sidebar/mod.rs"]
pub mod right_sidebar;
pub mod settings;
pub mod shared;
pub mod titlebar;

use shared::AppState;
use tauri::Manager;

/// Applies Windows-specific DWM attributes for sharp corners and no light border outline.
pub fn apply_window_dwm_styling(window: &tauri::WebviewWindow) {
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
            left_sidebar::set_active_session,
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
            main_content::messages::approve_permission,
            main_content::messages::deny_permission,
            // Main Content Markdown actions
            main_content::markdown::render_markdown,
            main_content::markdown::render_markdown_batch,
            // Right Sidebar (Source Control & Git Diff) actions
            right_sidebar::get_git_diff_details,
            right_sidebar::get_git_commit_graph,
            right_sidebar::get_workspace_repositories,
            right_sidebar::git_stage_file,
            right_sidebar::git_unstage_file,
            right_sidebar::git_revert_file,
            right_sidebar::git_stage_all_files,
            right_sidebar::git_unstage_all_files,
            right_sidebar::git_revert_all_files,
            right_sidebar::git_commit_changes,
            right_sidebar::git_generate_commit_message,
            right_sidebar::git_push_changes,
            right_sidebar::git_pull_changes,
            right_sidebar::git_fetch_changes,
            right_sidebar::git_create_branch,
            right_sidebar::git_switch_branch,
            right_sidebar::git_delete_branch,
            // Settings Window actions
            settings::open_settings_window,
            settings::close_settings_window,
            settings::general::get_general_settings,
            settings::general::save_general_settings,
            settings::appearance::get_appearance_settings,
            settings::appearance::save_appearance_settings,
            settings::models::get_providers_list,
            settings::models::get_provider_setup_details,
            settings::models::discover_provider_models,
            settings::models::save_provider_config,
            settings::permissions::get_allowed_directories,
            settings::permissions::add_allowed_directory,
            settings::permissions::remove_allowed_directory,
            settings::permissions::pick_allowed_directory_dialog,
            settings::permissions::get_permission_items,
            settings::permissions::update_permission_mode,
            settings::channels::get_channels_list,
            settings::channels::whatsapp::get_whatsapp_state,
            settings::channels::whatsapp::check_whatsapp_policy_coverage,
            settings::channels::whatsapp::pick_whatsapp_workspace_dialog,
            settings::channels::whatsapp::save_whatsapp_channel_config,
            settings::channels::whatsapp::start_whatsapp_qr_pairing,
            settings::channels::whatsapp::start_whatsapp_code_pairing,
            settings::channels::telegram::get_telegram_state,
            settings::channels::telegram::check_telegram_policy_coverage,
            settings::channels::telegram::pick_telegram_workspace_dialog,
            settings::channels::telegram::test_telegram_channel_connection,
            settings::channels::telegram::save_telegram_channel_config,
            settings::about::get_about_system_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
