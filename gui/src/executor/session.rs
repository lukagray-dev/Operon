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
    };

    let store = operon_rs::session::store::SessionStore::open(&store_path).await?;
    
    if is_new_session {
        store.create_session(
            session_id,
            &config.workspace_root.to_string_lossy(),
            config.provider_config.model_id(),
            &format!("{:?}", config.provider_config.provider),
        ).await?;
    }

    let history_turns = store.load_turns(session_id).await?;
    let turn_index = history_turns.len();
    let last_token_count = store.get_last_token_count(session_id).await?;

    let mut runner = operon_rs::session::SessionRunner::new(config, event_tx, cmd_rx).await?;
    if !history_turns.is_empty() {
        let history = history_turns.last().cloned().unwrap_or_default();
        runner.set_history(history, turn_index, last_token_count);
    }

    Ok((runner, turn_index, last_token_count))
}
