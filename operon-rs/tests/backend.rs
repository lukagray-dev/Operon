use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use operon_rs::{
    CallerRole, DirTool, DirectoryPolicy, Dispatcher, FsTool, GlobalPolicy, PermissionMode,
    PolicyConfig, PolicyResolver, Provider, ToolCall, ToolCallId, ToolContent, ToolProgress,
    ToolProgressEmitter, ToolProgressStage,
};
use serde_json::json;
use tempfile::tempdir;

fn tool_call(name: &str, call_id: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: ToolCallId(call_id.to_string()),
        name: name.to_string(),
        arguments,
    }
}

fn allowed_directory_policy(path: PathBuf) -> PolicyConfig {
    let owner = HashMap::from([
        (DirTool::Fs(FsTool::Read), PermissionMode::Allow),
        (DirTool::Fs(FsTool::Write), PermissionMode::Allow),
    ]);

    let external = HashMap::from([
        (DirTool::Fs(FsTool::Read), PermissionMode::Deny),
        (DirTool::Fs(FsTool::Write), PermissionMode::Deny),
    ]);

    PolicyConfig {
        global: GlobalPolicy::default(),
        directories: vec![DirectoryPolicy {
            path,
            owner,
            external,
        }],
    }
}

fn progress_sink() -> (ToolProgressEmitter, Arc<Mutex<Vec<ToolProgress>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink: ToolProgressEmitter = {
        let seen = Arc::clone(&seen);
        Arc::new(move |progress: ToolProgress| {
            seen.lock().unwrap().push(progress);
        })
    };
    (sink, seen)
}

/// A tiny environment guard used by the live config-load test.
///
/// The loader reads process environment variables, so the test needs a safe way
/// to swap them temporarily and restore whatever was there before once the test
/// finishes. The lock keeps the mutation serialized inside this test binary.
struct TempEnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(String, Option<String>)>,
}

impl TempEnvGuard {
    fn new(vars: &[(&str, Option<&str>)]) -> Self {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("environment lock should not be poisoned");

        let mut saved = Vec::with_capacity(vars.len());
        for (name, value) in vars {
            saved.push(((*name).to_string(), env::var(name).ok()));
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }

        Self { _lock: lock, saved }
    }
}

impl Drop for TempEnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.saved.iter().rev() {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}

/// Snapshot of the real ~/.operon state so the live config test can restore it.
///
/// We use the actual home directory on Windows because `dirs::home_dir()` resolves
/// through the system profile folder there, not through HOME/USERPROFILE overrides.
/// The guard keeps the test honest while still cleaning up after itself.
struct HomeStateGuard {
    paths: operon_rs::OperonPaths,
    config_dir_existed: bool,
    workspace_dir_existed: bool,
    sessions_dir_existed: bool,
    original_config: Option<Vec<u8>>,
}

impl HomeStateGuard {
    fn snapshot(paths: &operon_rs::OperonPaths) -> Self {
        Self {
            paths: paths.clone(),
            config_dir_existed: paths.config_dir.exists(),
            workspace_dir_existed: paths.workspace_dir.exists(),
            sessions_dir_existed: paths.sessions_dir.exists(),
            original_config: fs::read(&paths.config_file).ok(),
        }
    }
}

impl Drop for HomeStateGuard {
    fn drop(&mut self) {
        if let Some(original) = &self.original_config {
            let _ = fs::write(&self.paths.config_file, original);
        } else {
            let _ = fs::remove_file(&self.paths.config_file);
        }

        if !self.workspace_dir_existed && self.paths.workspace_dir.exists() {
            let _ = fs::remove_dir_all(&self.paths.workspace_dir);
        }

        if !self.sessions_dir_existed && self.paths.sessions_dir.exists() {
            let _ = fs::remove_dir_all(&self.paths.sessions_dir);
        }

        if !self.config_dir_existed && self.paths.config_dir.exists() {
            let _ = fs::remove_dir_all(&self.paths.config_dir);
        }
    }
}

#[test]
fn facade_exports_backend_surface() {
    // This is the integration smoke test for the public facade.
    let _load_fn: fn() -> Result<operon_rs::AppConfig, operon_rs::ConfigError> = operon_rs::load;

    let _ = operon_rs::SessionCommand::Cancel;
    let _ = operon_rs::SessionEvent::SessionStarted {
        session_id: "session-smoke".to_string(),
    };
    let _ = operon_rs::Provider::Anthropic;
    let _ = ToolProgress::completed(
        ToolCallId("call_smoke".to_string()),
        "write",
        Some("/tmp/file.txt".to_string()),
        "completed",
    );
    let _ = ToolCall {
        id: ToolCallId("call_1".to_string()),
        name: "read".to_string(),
        arguments: json!({ "paths": "/tmp/file.txt" }),
    };
    let _ = ToolContent::Text("hello".to_string());
    let _ = ToolProgressStage::Running;
}

#[test]
#[ignore = "requires OPERON_TEST_API_KEY to be set for live config resolution"]
fn load_resolves_api_key_end_to_end() {
    // This test exercises the full config path:
    // config.toml -> operon-config loader -> ProviderConfig -> facade output.
    // It uses a real API key only through the environment, never by embedding
    // the secret directly in the repository.
    let live_key = env::var("OPERON_TEST_API_KEY")
        .expect("set OPERON_TEST_API_KEY to a real key before running this test");

    let paths = operon_rs::OperonPaths::resolve().expect("home directory should resolve");
    let _state_guard = HomeStateGuard::snapshot(&paths);
    let _env_guard = TempEnvGuard::new(&[("OPENAI_API_KEY", Some(live_key.as_str()))]);

    // Keep the config intentionally minimal: the loader should still resolve the
    // env key, inject the workspace, and build the runtime config correctly.
    fs::create_dir_all(&paths.config_dir).unwrap();
    fs::write(
        &paths.config_file,
        r#"
[provider]
name           = "open_ai"
model_id       = "openai/gpt-oss120b"
context_window = 131072
max_tokens     = 8192

[credentials]
api_key = ""

[policy.global.owner]
web = "allow"
"#,
    )
    .unwrap();

    let loaded = operon_rs::load().expect("config should load successfully");

    assert_eq!(loaded.provider.provider, Provider::OpenAI);
    assert_eq!(loaded.provider.model_id(), "openai/gpt-oss120b");
    assert_eq!(loaded.provider.context_window(), 131072);
    assert_eq!(loaded.provider.max_tokens(), 8192);
    assert!(
        loaded.provider.credentials.has_key(),
        "API key should be resolved from the environment"
    );
    assert!(
        loaded.provider.credentials.api_key.expose() == live_key,
        "resolved API key should match the supplied environment value"
    );
    assert!(
        loaded.paths.config_file.exists(),
        "config file should exist"
    );
    assert!(
        loaded.paths.workspace_dir.is_dir(),
        "workspace dir should be created"
    );
    assert!(
        loaded.paths.sessions_dir.is_dir(),
        "sessions dir should be created"
    );
}

#[test]
fn policy_resolver_enforces_directory_permissions() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("notes.txt");
    fs::write(&file_path, "hello").unwrap();

    let mut policy = allowed_directory_policy(fs::canonicalize(temp.path()).unwrap());
    policy.validate().unwrap();

    let resolver = PolicyResolver::new(policy);
    let call = tool_call(
        "read",
        "call_read",
        json!({ "paths": file_path.to_string_lossy().to_string() }),
    );

    let owner_decision = resolver.check(&call, CallerRole::Owner);
    assert!(owner_decision.is_allow(), "owner should be able to read");

    let external_decision = resolver.check(&call, CallerRole::External);
    assert!(
        external_decision.is_deny(),
        "external callers should be denied by this policy"
    );
}

#[tokio::test]
async fn dispatcher_enforces_read_before_write_and_emits_progress() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("report.txt");
    fs::write(&file_path, "original").unwrap();

    let mut dispatcher = Dispatcher::new();
    dispatcher.register_fs_tools();

    // The dispatcher enforces read-before-write on existing files.
    let denied_write = tool_call(
        "write",
        "call_write_denied",
        json!({
            "path": file_path.to_string_lossy().to_string(),
            "__body__": "updated"
        }),
    );
    let denied_result = dispatcher.dispatch(denied_write).await;
    assert!(
        denied_result.is_error,
        "write should be blocked before read"
    );
    assert!(
        matches!(denied_result.content, ToolContent::Text(ref text) if text.contains("read-before-write")),
        "read-before-write enforcement should be visible in the error text"
    );

    let (sink, seen) = progress_sink();

    let read_call = tool_call(
        "read",
        "call_read",
        json!({ "paths": file_path.to_string_lossy().to_string() }),
    );
    let read_outcome = dispatcher
        .dispatch_with_progress(read_call, Some(sink.clone()))
        .await;
    assert!(!read_outcome.result.is_error, "read should succeed");

    let write_call = tool_call(
        "write",
        "call_write",
        json!({
            "path": file_path.to_string_lossy().to_string(),
            "__body__": "updated"
        }),
    );
    let write_outcome = dispatcher
        .dispatch_with_progress(write_call, Some(sink))
        .await;
    assert!(
        !write_outcome.result.is_error,
        "write should succeed after read"
    );

    let written = fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, "updated");

    let captured = seen.lock().unwrap();
    assert!(
        captured
            .iter()
            .any(|p| p.tool == "read" && p.stage == ToolProgressStage::Running),
        "read should emit a running progress update"
    );
    assert!(
        captured
            .iter()
            .any(|p| p.tool == "write" && p.stage == ToolProgressStage::Running),
        "write should emit a running progress update"
    );
    assert!(
        captured
            .iter()
            .any(|p| p.tool == "write" && p.stage == ToolProgressStage::Completed),
        "write should emit a completed progress update"
    );
}
