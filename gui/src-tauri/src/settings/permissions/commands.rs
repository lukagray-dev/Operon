//! Permissions Settings Backend Tauri Commands.
//
// 1:1 match with Slint settings/main-content/permissions.rs:
// - Loads allowed directories list and workspace root.
// - Supports adding, picking, and removing allowed directory scopes.
// - Retrieves granular tool/group permission rows for "owner" and "external" scopes.
// - Persists permission overrides to ~/.operon/config.toml via operon_rs.

use super::types::{AllowedDirectoriesDto, PermissionItemDto, UpdatePermissionRequestDto};

/// Cleans Windows raw UNC path prefixes.
fn clean_windows_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else if let Some(stripped) = path.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path.to_string()
    }
}

/// Retrieves allowed directories and workspace directory.
#[tauri::command]
pub async fn get_allowed_directories() -> Result<AllowedDirectoriesDto, String> {
    match operon_rs::get_allowed_directories_list() {
        Ok((directories, workspace_directory)) => {
            let mut dirs_list: Vec<String> = directories
                .into_iter()
                .map(|d| clean_windows_path(&d))
                .collect();

            let cleaned_workspace = clean_windows_path(&workspace_directory);

            if !cleaned_workspace.is_empty() && !dirs_list.contains(&cleaned_workspace) {
                dirs_list.insert(0, cleaned_workspace.clone());
            }

            Ok(AllowedDirectoriesDto {
                directories: dirs_list,
                workspace_directory: cleaned_workspace,
            })
        }
        Err(e) => Err(format!("Failed to load allowed directories: {:#}", e)),
    }
}

/// Adds a new path to the list of allowed directories.
#[tauri::command]
pub async fn add_allowed_directory(path: String) -> Result<(), String> {
    let clean = path.trim();
    if clean.is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }
    operon_rs::add_allowed_directory(clean).map_err(|e| e.to_string())
}

/// Removes a directory path from allowed directories.
#[tauri::command]
pub async fn remove_allowed_directory(path: String) -> Result<(), String> {
    let clean = path.trim();
    if clean.is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }
    operon_rs::remove_allowed_directory(clean).map_err(|e| e.to_string())
}

/// Opens native folder picker dialog to select an allowed directory.
#[tauri::command]
pub async fn pick_allowed_directory_dialog() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Allowed Directory")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Retrieves granular group and tool permission rows for a scope and optional directory.
#[tauri::command]
pub async fn get_permission_items(
    scope: String,
    directory: Option<String>,
) -> Result<Vec<PermissionItemDto>, String> {
    let dir_param = directory.as_deref().filter(|d| !d.trim().is_empty());
    let rows = operon_rs::get_permission_rows(&scope, dir_param).map_err(|e| e.to_string())?;

    let mut groups = Vec::new();
    let mut tools = Vec::new();

    for r in rows {
        if r.kind == "group" {
            groups.push(r);
        } else {
            tools.push(r);
        }
    }

    let mut items = Vec::new();
    for g in groups {
        let has_tools = tools.iter().any(|t| t.group_key == g.key);

        items.push(PermissionItemDto {
            key: g.key.clone(),
            label: g.label.clone(),
            subtitle: format!("group key: {} • default: {}", g.key, g.base_mode),
            mode: g.mode.clone(),
            base_mode: g.base_mode.clone(),
            is_explicit: g.is_explicit,
            kind: g.kind.clone(),
            group_key: g.group_key.clone(),
            is_expanded: false,
            has_tools,
        });

        for t in &tools {
            if t.group_key == g.key {
                items.push(PermissionItemDto {
                    key: t.key.clone(),
                    label: t.label.clone(),
                    subtitle: format!("tool key: {} • default: {}", t.key, t.base_mode),
                    mode: t.mode.clone(),
                    base_mode: t.base_mode.clone(),
                    is_explicit: t.is_explicit,
                    kind: t.kind.clone(),
                    group_key: t.group_key.clone(),
                    is_expanded: false,
                    has_tools: false,
                });
            }
        }
    }

    Ok(items)
}

/// Updates permission mode (Allow, Ask, Deny) for a group or tool.
#[tauri::command]
pub async fn update_permission_mode(request: UpdatePermissionRequestDto) -> Result<(), String> {
    let dir_param = request.directory.as_deref().filter(|d| !d.trim().is_empty());
    let mut target_mode = Some(request.mode.as_str());

    // Check if target matches default base mode to clear override
    if let Ok(rows) = operon_rs::get_permission_rows(&request.scope, dir_param) {
        if let Some(row) = rows
            .iter()
            .find(|r| r.key == request.key && r.kind == request.kind)
        {
            if row.base_mode == request.mode {
                target_mode = None;
            }
        }
    }

    operon_rs::update_permission(&request.scope, dir_param, &request.key, target_mode)
        .map_err(|e| e.to_string())
}
