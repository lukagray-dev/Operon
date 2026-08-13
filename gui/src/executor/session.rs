//! Session initialization and configuration setup.
//!
//! Hey friend! This file manages loading configuration, checking session database state,
//! and initializing the SessionRunner structure. Keeping it here avoids cluttering mod.rs.

/// Setup and initialize the SessionRunner.
///
/// Loads the configuration path, creates the db if new, loads history, and instantiates the runner.
pub async fn start_agent_session(
    session_id: &str,
    is_new_session: bool,
    project_dir: Option<String>,
    event_tx: tokio::sync::mpsc::Sender<operon_rs::SessionEvent>,
    cmd_rx: tokio::sync::mpsc::Receiver<operon_rs::SessionCommand>,
) -> anyhow::Result<(operon_rs::session::SessionRunner, usize, Option<usize>)> {
    let app_config = operon_rs::load()?;

    let workspace_root = if let Some(ref proj) = project_dir {
        std::path::PathBuf::from(proj)
    } else {
        app_config.paths.workspace_dir.clone()
    };

    let store_path = app_config.paths.session_db(session_id);

    let config = operon_rs::session::SessionConfig {
        provider_config: app_config.provider.clone(),
        policy: app_config.policy.clone(),
        project_dir: project_dir.map(std::path::PathBuf::from),
        workspace_root,
        role: operon_rs::context::Role::Owner,
        tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
        compaction: operon_rs::context::CompactionConfig::default(),
        store_path: Some(store_path.clone()),
        channel_instructions: None,
    };

    let store = operon_rs::session::store::SessionStore::open(&store_path).await?;

    if is_new_session {
        store
            .create_session(
                session_id,
                &config.workspace_root.to_string_lossy(),
                config.provider_config.model_id(),
                &format!("{:?}", config.provider_config.provider),
            )
            .await?;
    }

    let history = store.load_full_history(session_id).await?;
    let history_turns = store.load_turns(session_id).await?;
    let turn_index = history_turns.len();
    let last_token_count = store.get_last_token_count(session_id).await?;

    let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx).await?;
    if !history.is_empty() {
        runner.set_history(history, turn_index, last_token_count);
    }

    Ok((runner, turn_index, last_token_count))
}

/// Helper function to strip raw Windows UNC prefix for cleaner paths.
pub fn clean_unc_path(s: String) -> String {
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s
    }
}

/// Queries all session JSON files and filters/groups them for the sidebar lists.
///
/// Hey friend! This reads allowed directories, scans the sessions directory,
/// filters them using the search query, sorts them newest-first, and divides
/// them into standalone chats vs project conversations.
pub async fn query_sidebar_data(
    search_query: String,
) -> anyhow::Result<(
    Vec<crate::SidebarConversation>,
    Vec<(String, String, Vec<crate::SidebarConversation>)>,
)> {
    let paths = operon_rs::config::OperonPaths::resolve()?;
    let sessions_dir = paths.sessions_dir;

    let default_workspace = {
        let p = paths
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| paths.workspace_dir.clone())
            .to_string_lossy()
            .to_string();
        clean_unc_path(p)
    };

    // Query configured workspace directories from config.toml
    let mut projects_list = Vec::new();
    if let Ok(allowed_dirs) = operon_rs::get_allowed_directories_list() {
        for dir in allowed_dirs.0 {
            let cleaned = clean_unc_path(dir.clone());
            if cleaned != default_workspace {
                projects_list.push(dir);
            }
        }
    }

    struct SessionRecord {
        id: String,
        created_at: i64,
        workspace: String,
        title: String,
        is_project: bool,
    }

    let mut sessions = Vec::new();
    if sessions_dir.exists() {
        let entries = std::fs::read_dir(sessions_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(store) = operon_rs::session::store::SessionStore::open(&path).await {
                    if let Ok(rows) = store.list_sessions().await {
                        if let Some(row) = rows.first() {
                            let first_msg = store
                                .get_first_user_message_text(&row.id)
                                .await
                                .ok()
                                .flatten();
                            let title = crate::main_content::title::determine_session_title(
                                first_msg.as_deref(),
                                "Untitled Chat",
                            );

                            let session_workspace_canon = {
                                let p = std::path::PathBuf::from(&row.workspace)
                                    .canonicalize()
                                    .unwrap_or_else(|_| std::path::PathBuf::from(&row.workspace))
                                    .to_string_lossy()
                                    .to_string();
                                clean_unc_path(p)
                            };

                            let is_project = session_workspace_canon != default_workspace;
                            let project_name = if is_project {
                                std::path::Path::new(&row.workspace)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                String::new()
                            };

                            // Filter by search query if present
                            let matches_search = search_query.is_empty()
                                || title.to_lowercase().contains(&search_query)
                                || project_name.to_lowercase().contains(&search_query);

                            if matches_search {
                                sessions.push(SessionRecord {
                                    id: row.id.clone(),
                                    created_at: row.created_at,
                                    workspace: row.workspace.clone(),
                                    title,
                                    is_project,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort newest first
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Separate into standalone chats vs project conversations
    let mut standalone_chats = Vec::new();
    let mut project_chats_map: std::collections::HashMap<String, Vec<crate::SidebarConversation>> =
        std::collections::HashMap::new();

    for p in &projects_list {
        project_chats_map.insert(p.clone(), Vec::new());
    }

    for s in &sessions {
        if !s.is_project {
            standalone_chats.push(crate::SidebarConversation {
                id: s.id.clone().into(),
                title: s.title.clone().into(),
            });
        } else {
            let entry_chats = project_chats_map
                .entry(s.workspace.clone())
                .or_insert_with(Vec::new);
            entry_chats.push(crate::SidebarConversation {
                id: s.id.clone().into(),
                title: s.title.clone().into(),
            });
        }
    }

    // Group project details
    let mut projects_data = Vec::new();
    for p in projects_list {
        let name = std::path::Path::new(&p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&p)
            .to_string();

        let conversations = project_chats_map.remove(&p).unwrap_or_default();

        let project_matches = search_query.is_empty()
            || name.to_lowercase().contains(&search_query)
            || !conversations.is_empty();

        if project_matches {
            projects_data.push((name, p, conversations));
        }
    }

    Ok((standalone_chats, projects_data))
}
