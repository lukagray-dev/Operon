//! Central JSON-RPC method dispatch router.
//!
//! Maps JSON-RPC request methods to their corresponding async handlers in the bridge codebase.
//! All deserialization errors and execution errors are returned as cleanly structured JSON values
//! or user-friendly error strings.

use std::sync::Arc;
use serde_json::Value;

use crate::left_sidebar;
use crate::main_content;
use crate::right_sidebar;
use crate::settings;
use crate::shared::AppState;

/// Dispatches an incoming JSON-RPC method call to the appropriate module handler.
pub async fn dispatch(
    method: &str,
    params: Value,
    state: &Arc<AppState>,
) -> Result<Value, String> {
    match method {
        // =========================================================================
        // Settings - General
        // =========================================================================
        "get_general_settings" => {
            let res = settings::general::get_general_settings().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "save_general_settings" => {
            let dto: settings::general::types::GeneralSettingsDto =
                serde_json::from_value(params.get("settings").cloned().unwrap_or(params))
                    .map_err(|e| format!("Invalid GeneralSettingsDto params: {e}"))?;
            settings::general::save_general_settings(dto, state).await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Settings - Appearance
        // =========================================================================
        "get_appearance_settings" => {
            let res = settings::appearance::get_appearance_settings().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "save_appearance_settings" => {
            let dto: settings::appearance::types::AppearanceSettingsDto =
                serde_json::from_value(params.get("settings").cloned().unwrap_or(params))
                    .map_err(|e| format!("Invalid AppearanceSettingsDto params: {e}"))?;
            settings::appearance::save_appearance_settings(dto, state).await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Settings - Models & Providers
        // =========================================================================
        "get_providers_list" => {
            let res = settings::models::get_providers_list().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "get_provider_setup_details" => {
            let provider_id = params
                .get("providerId")
                .or_else(|| params.get("provider_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing 'providerId' parameter".to_string())?
                .to_string();
            let res = settings::models::get_provider_setup_details(provider_id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "discover_provider_models" => {
            let provider_id = params
                .get("providerId")
                .or_else(|| params.get("provider_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let api_base = params
                .get("apiBase")
                .or_else(|| params.get("api_base"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let api_key = params
                .get("apiKey")
                .or_else(|| params.get("api_key"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = settings::models::discover_provider_models(provider_id, api_base, api_key).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "save_provider_config" => {
            let request: settings::models::types::SaveProviderRequestDto =
                serde_json::from_value(params.get("request").cloned().unwrap_or(params))
                    .map_err(|e| format!("Invalid SaveProviderRequestDto: {e}"))?;
            settings::models::save_provider_config(request).await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Settings - Permissions
        // =========================================================================
        "get_allowed_directories" => {
            let res = settings::permissions::get_allowed_directories().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "add_allowed_directory" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing 'path' parameter".to_string())?
                .to_string();
            settings::permissions::add_allowed_directory(path).await?;
            Ok(Value::Null)
        }
        "remove_allowed_directory" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing 'path' parameter".to_string())?
                .to_string();
            settings::permissions::remove_allowed_directory(path).await?;
            Ok(Value::Null)
        }
        "pick_allowed_directory_dialog" => {
            let res = settings::permissions::pick_allowed_directory_dialog().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "get_permission_items" => {
            let scope = params
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("owner")
                .to_string();
            let directory = params
                .get("directory")
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = settings::permissions::get_permission_items(scope, directory).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "update_permission_mode" => {
            let request: settings::permissions::types::UpdatePermissionRequestDto =
                serde_json::from_value(params.get("request").cloned().unwrap_or(params))
                    .map_err(|e| format!("Invalid UpdatePermissionRequestDto: {e}"))?;
            settings::permissions::update_permission_mode(request).await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Settings - Memory
        // =========================================================================
        "memory_list" => {
            let limit = params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            let offset = params
                .get("offset")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let res = settings::memory::memory_list(limit, offset).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "memory_add" => {
            let content = params
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tags: Vec<String> = params
                .get("tags")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let res = settings::memory::memory_add(content, tags).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "memory_edit" => {
            let id = params
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = params.get("content").and_then(|v| v.as_str()).map(String::from);
            let tags: Option<Vec<String>> = params
                .get("tags")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let res = settings::memory::memory_edit(id, content, tags).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "memory_delete" => {
            let id = params
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = settings::memory::memory_delete(id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }

        // =========================================================================
        // Settings - About & Window
        // =========================================================================
        "get_about_system_info" => {
            let res = settings::about::get_about_system_info().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "open_settings_window" => {
            settings::open_settings_window().await?;
            Ok(Value::Null)
        }
        "close_settings_window" => {
            settings::close_settings_window().await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Left Sidebar - Sessions & Projects
        // =========================================================================
        "query_sidebar_data" => {
            let search_query = params
                .get("searchQuery")
                .or_else(|| params.get("search_query"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = left_sidebar::query_sidebar_data(search_query, state).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "delete_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            left_sidebar::delete_session(session_id).await?;
            Ok(Value::Null)
        }
        "delete_project" => {
            let project_path = params
                .get("projectPath")
                .or_else(|| params.get("project_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            left_sidebar::delete_project(project_path).await?;
            Ok(Value::Null)
        }
        "open_project_picker" => {
            let res = left_sidebar::open_project_picker().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "create_new_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let project_path = params
                .get("projectPath")
                .or_else(|| params.get("project_path"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = left_sidebar::create_new_session(session_id, project_path, state).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "set_active_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let project_path = params
                .get("projectPath")
                .or_else(|| params.get("project_path"))
                .and_then(|v| v.as_str())
                .map(String::from);
            left_sidebar::set_active_session(session_id, project_path, state).await?;
            Ok(Value::Null)
        }
        "rename_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let new_title = params
                .get("newTitle")
                .or_else(|| params.get("new_title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            left_sidebar::rename_session(session_id, new_title).await?;
            Ok(Value::Null)
        }
        "fork_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let until_turn_index = params
                .get("untilTurnIndex")
                .or_else(|| params.get("until_turn_index"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let res = left_sidebar::fork_session(session_id, until_turn_index).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "move_session" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_workspace = params
                .get("targetWorkspace")
                .or_else(|| params.get("target_workspace"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            left_sidebar::move_session(session_id, target_workspace).await?;
            Ok(Value::Null)
        }

        // =========================================================================
        // Right Sidebar - Todos / Tasks
        // =========================================================================
        "get_session_todos" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = right_sidebar::get_session_todos(session_id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "update_session_todo_status" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let todo_id = params
                .get("todoId")
                .or_else(|| params.get("todo_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status = params
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = right_sidebar::update_session_todo_status(session_id, todo_id, status).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "delete_session_todo" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let todo_id = params
                .get("todoId")
                .or_else(|| params.get("todo_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = right_sidebar::delete_session_todo(session_id, todo_id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "create_session_todo" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = params
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let priority = params
                .get("priority")
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = right_sidebar::create_session_todo(session_id, content, priority).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }

        // =========================================================================
        // Main Content - Topbar, Input, Messages, Markdown
        // =========================================================================
        "get_topbar_info" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let workspace_path = params
                .get("workspacePath")
                .or_else(|| params.get("workspace_path"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = main_content::topbar::get_topbar_info(session_id, workspace_path).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "load_session_messages" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = main_content::messages::load_session_messages(session_id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "submit_prompt" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let prompt = params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let attachments: Vec<main_content::input::PendingAttachmentDto> = params
                .get("attachments")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let workspace_path = params
                .get("workspacePath")
                .or_else(|| params.get("workspace_path"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = main_content::messages::submit_prompt(
                state,
                session_id,
                prompt,
                attachments,
                workspace_path,
            )
            .await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "cancel_prompt" => {
            main_content::messages::cancel_prompt().await?;
            Ok(Value::Null)
        }
        "edit_and_submit_prompt" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt = params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_turn_index = params
                .get("targetTurnIndex")
                .or_else(|| params.get("target_turn_index"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let workspace_path = params
                .get("workspacePath")
                .or_else(|| params.get("workspace_path"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = main_content::messages::edit_and_submit_prompt(
                state,
                session_id,
                prompt,
                target_turn_index,
                workspace_path,
            )
            .await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "approve_permission" => {
            let permission_id = params
                .get("permissionId")
                .or_else(|| params.get("permission_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            main_content::messages::approve_permission(permission_id).await?;
            Ok(Value::Null)
        }
        "deny_permission" => {
            let permission_id = params
                .get("permissionId")
                .or_else(|| params.get("permission_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            main_content::messages::deny_permission(permission_id).await?;
            Ok(Value::Null)
        }
        "get_pending_permissions" => {
            let perms = crate::shared::channels_manager::get_all_pending_permissions();
            serde_json::to_value(perms).map_err(|e| e.to_string())
        }
        "respond_to_ask" => {
            let id = params
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let answer = params
                .get("answer")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            main_content::messages::respond_to_ask(id, answer).await?;
            Ok(Value::Null)
        }
        "get_available_models" => {
            let res = main_content::input::get_available_models().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "select_model" => {
            let model_id = params
                .get("modelId")
                .or_else(|| params.get("model_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let reasoning = params
                .get("reasoning")
                .and_then(|v| v.as_str())
                .map(String::from);
            let context_window = params
                .get("contextWindow")
                .or_else(|| params.get("context_window"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            main_content::input::select_model(model_id, reasoning, context_window).await?;
            Ok(Value::Null)
        }
        "toggle_auto_approve" => {
            let enabled = params
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let res = main_content::input::toggle_auto_approve(enabled, state).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "pick_attachments_dialog" => {
            let res = main_content::input::pick_attachments_dialog().await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "get_context_window_info" => {
            let session_id = params
                .get("sessionId")
                .or_else(|| params.get("session_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let res = main_content::input::get_context_window_info(state, session_id).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "render_markdown" => {
            let markdown = params
                .get("markdown")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let res = main_content::markdown::render_markdown(markdown).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "render_markdown_batch" => {
            let texts: Vec<String> = params
                .get("texts")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let res = main_content::markdown::render_markdown_batch(texts).await?;
            serde_json::to_value(res).map_err(|e| e.to_string())
        }
        "send_desktop_notification" => Ok(Value::Null),

        unknown => Err(format!("Unknown JSON-RPC method: '{unknown}'")),
    }
}
