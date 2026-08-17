// loader.rs — Config file loading, env var overrides, and AppConfig construction.
//
// The main entry point is `load()`:
//   1. Resolve platform paths (OperonPaths::resolve()).
//   2. Create ~/.operon/, workspace/, sessions/ if missing (ensure_dirs_exist()).
//   3. Read config.toml — write the default if it doesn't exist yet.
//   4. Parse TOML into AppConfigToml.
//   5. Apply env var overrides (API key from provider-specific env var).
//   6. Build ProviderConfig and validate credentials.
//   7. Build PolicyConfig (global + directories) + always inject the default workspace.
//   8. Call PolicyConfig::validate() to canonicalize all directory paths.
//   9. Return AppConfig.
//
// DEFAULT CONFIG BEHAVIOR:
//   On first run, no config.toml exists. The loader writes a documented
//   scaffold with empty provider/model fields and commented guidance. The user
//   edits this file to choose a provider, add credentials, and set policies.
//
// ENV VAR OVERRIDE ORDER:
//   1. [credentials] api_key in config.toml — explicit file-based credential.
//   2. Provider-specific env var (e.g. ANTHROPIC_API_KEY) — CI/container friendly.
//   If both are set, the config file wins (allows secrets in file without env pollution).
//   If neither is set, MissingApiKey error (except Ollama which is auth-free).

use std::fs;

use crate::error::ConfigError;
use crate::paths::OperonPaths;
use crate::policy::{DirectoryPolicy, PolicyConfig};
use crate::schema::{
    build_directory_policy, build_global_policy, build_provider_config, AppConfig, AppConfigToml,
};
use serde::de::Error as _;
use toml_edit::{value, DocumentMut, Item, Table};

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Load, validate, and return the complete Operon configuration.
///
/// This is the only function consumers need. All Operon binaries (TUI, GUI,
/// CLI) call this exactly once at startup and pass the resulting `AppConfig`
/// to `SessionRunner::new()`.
///
/// # What this does
///
/// 1. Finds `~/.operon/config.toml` using platform-safe home directory detection.
/// 2. Creates `~/.operon/workspace/` and `~/.operon/sessions/` if they don't exist.
/// 3. Writes a default `config.toml` if none exists yet (first run).
/// 4. Parses the TOML, applies env var overrides, and validates all paths.
/// 5. Ensures the default workspace (`~/.operon/workspace/`) is always in the
///    policy's allowed directory list with owner-full-access permissions.
///
/// # Errors
///
/// Returns `ConfigError` for any failure: missing home dir, I/O error,
/// malformed TOML, missing provider/model selection, unknown provider name,
/// missing API key, or a directory path in `[[directories]]` that cannot be
/// canonicalized.
pub fn load() -> Result<AppConfig, ConfigError> {
    // Step 1: Resolve all runtime paths.
    let paths = OperonPaths::resolve()?;

    // Step 2: Create directories if needed (idempotent).
    paths.ensure_dirs_exist()?;

    // Step 3: Read or create config.toml.
    let toml_text = read_or_create_config(&paths)?;

    // Step 4: Parse TOML.
    let toml_config: AppConfigToml =
        toml::from_str(&toml_text).map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: e,
        })?;

    // Step 5: Resolve the API key (config file first, then env var).
    let env_api_key = resolve_env_api_key(&toml_config.provider.name);

    // Step 6: Build ProviderConfig and validate credentials.
    let provider =
        build_provider_config(&toml_config.provider, &toml_config.credentials, env_api_key)?;

    // Step 7a: Check that a key exists for providers that require one.
    validate_credentials(&provider.provider, &provider.credentials.api_key)?;

    // Step 7b: Build GlobalPolicy from TOML.
    let global = build_global_policy(&toml_config.policy.global);

    // Step 7c: Build DirectoryPolicy list.
    let mut directories: Vec<DirectoryPolicy> = toml_config
        .directories
        .iter()
        .map(build_directory_policy)
        .collect();

    // Step 7d: Always inject the default workspace directory (Direction 1).
    // The workspace is always accessible to the agent — the user cannot remove it.
    // We inject with full owner access and zero external access (safe default).
    inject_default_workspace(&mut directories, &paths);

    // Step 8: Assemble and validate the PolicyConfig.
    let mut policy = PolicyConfig {
        global,
        directories,
    };
    policy.validate()?;

    Ok(AppConfig {
        provider,
        policy,
        paths,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Reads `~/.operon/config.toml`. If it doesn't exist, writes the default
/// config file and returns the default content.
fn read_or_create_config(paths: &OperonPaths) -> Result<String, ConfigError> {
    if paths.config_file.exists() {
        // Normal path — read the existing file.
        Ok(fs::read_to_string(&paths.config_file)?)
    } else {
        // First run — write the default config so the user has an editable file.
        let default = default_config_content();
        fs::write(&paths.config_file, &default)?;
        Ok(default)
    }
}

/// Ensures the default workspace is in the `directories` list.
///
/// If it's already there (because the user added it manually), we leave their
/// entry unchanged so their custom permissions are preserved. If it's absent,
/// we inject it with `DirectoryPolicy::owner_full_access()` — full owner access,
/// no external access.
///
/// This implements the "user cannot remove Direction 1" rule from the design.
fn inject_default_workspace(directories: &mut Vec<DirectoryPolicy>, paths: &OperonPaths) {
    let workspace = &paths.workspace_dir;
    let already_present = directories.iter().any(|d| d.path == *workspace);
    if !already_present {
        directories.push(DirectoryPolicy::owner_full_access(workspace.clone()));
    }
}

/// Returns the provider-specific environment variable name for the API key.
fn api_key_env_var(provider_name: &str) -> &'static str {
    match provider_name {
        "anthropic" => "ANTHROPIC_API_KEY",
        "open_ai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        "ollama" => "", // Ollama is auth-free — no env var
        "deep_seek" => "DEEPSEEK_API_KEY",
        "open_router" => "OPENROUTER_API_KEY",
        "groq" => "GROQ_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "xai" => "XAI_API_KEY",
        "cohere" => "COHERE_API_KEY",
        _ => "", // Unknown provider — handled by parse_provider later
    }
}

/// Reads the provider-specific API key env var if set and non-empty.
fn resolve_env_api_key(provider_name: &str) -> Option<String> {
    let var = api_key_env_var(provider_name);
    if var.is_empty() {
        return None;
    }
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

/// Validates that the resolved credentials have a key for providers that need one.
///
/// Ollama is exempted — it runs locally and doesn't require authentication.
fn validate_credentials(
    provider: &operon_providers::Provider,
    api_key: &operon_providers::SecretString,
) -> Result<(), ConfigError> {
    use operon_providers::Provider;

    // Ollama is local — no API key needed.
    if *provider == Provider::Ollama {
        return Ok(());
    }

    if api_key.is_empty() {
        // Serialize the provider to get its snake_case name for the error message.
        let provider_name = serde_json::to_string(provider)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        let env_var = api_key_env_var(&provider_name).to_string();

        return Err(ConfigError::MissingApiKey {
            provider: provider_name,
            env_var,
        });
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Default config content
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the content to write as the default `config.toml` on first run.
///
/// The file is heavily commented so the user understands every option without
/// needing to read external documentation. Comments use the `#` prefix which
/// TOML preserves as-is in the file.
fn default_config_content() -> String {
    r#"# ~/.operon/config.toml — Operon main configuration file
#
# This file was created automatically on first run.
# Edit it to configure your provider, API key, and tool permissions.
# The provider and model fields start empty on purpose.
# Restart Operon after making changes.
#
# ─────────────────────────────────────────────────────────────────────────────
# PROVIDER
#
# Which LLM provider and model to use.
#
# name: snake_case provider identifier. Supported values:
#   anthropic, open_ai, gemini, ollama, deep_seek,
#   open_router, groq, mistral, xai, cohere
# ─────────────────────────────────────────────────────────────────────────────

[provider]
name           = ""
model_id       = ""
context_window = 0
max_tokens     = 0

# ─────────────────────────────────────────────────────────────────────────────
# CREDENTIALS
#
# API key for the configured provider.
# Leave api_key empty to load it from an environment variable instead:
#   Anthropic:   ANTHROPIC_API_KEY
#   OpenAI:      OPENAI_API_KEY
#   Gemini:      GEMINI_API_KEY
#   DeepSeek:    DEEPSEEK_API_KEY
#   OpenRouter:  OPENROUTER_API_KEY
#   Groq:        GROQ_API_KEY
#   Mistral:     MISTRAL_API_KEY
#   xAI:         XAI_API_KEY
#   Cohere:      COHERE_API_KEY
#   Ollama:      (no key needed — Ollama is local)
# ─────────────────────────────────────────────────────────────────────────────

[credentials]
api_key = ""
# org_id = ""  # OpenAI only: your organization ID for org-level billing

# ─────────────────────────────────────────────────────────────────────────────
# GLOBAL TOOL PERMISSIONS
#
# Permissions for tools that are not directory-scoped (web, subagent, etc.).
# Set per role: owner (you and trusted staff) vs. external (customers, public).
#
# Three modes:
#   allow — tool can be used freely
#   ask   — tool requires your confirmation before use
#   deny  — tool is blocked (default if not listed here)
# ─────────────────────────────────────────────────────────────────────────────

[policy.global.owner]
web        = "ask"
sub_agent  = "ask"
ask        = "ask"
todo       = "ask"
load_tools = "ask"

[policy.global.external]
web        = "deny"
sub_agent  = "deny"
ask        = "deny"
todo       = "deny"
load_tools = "deny"

# ─────────────────────────────────────────────────────────────────────────────
# ALLOWED DIRECTORIES
#
# Directories the agent can access via filesystem and shell tools.
# The default workspace (~/.operon/workspace/) is always allowed for the owner.
#
# Add as many [[directories]] sections as you need.
#
# PERMISSIONS:
#   fs   = "allow"/"ask"/"deny"  → applies to all 7 fs tools at once
#   bash = "allow"/"ask"/"deny"  → shell execution in this directory
#
# For per-tool overrides, use fs_read, fs_write, fs_edit, fs_append,
# fs_grep, fs_ls, fs_delete alongside (or instead of) the fs shorthand.
# Individual keys take precedence over the group shorthand.
# ─────────────────────────────────────────────────────────────────────────────

[[directories]]
path = "~/.operon/workspace"

[directories.permissions.owner]
fs   = "allow"
bash = "allow"

[directories.permissions.external]
fs   = "deny"
bash = "deny"
"#
    .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AppConfigToml;
    use operon_providers::{ApiCredentials, ModelConfig, Provider, ProviderConfig};

    #[test]
    fn test_default_config_parses_cleanly() {
        // The default config file content must parse without errors.
        let content = default_config_content();
        let parsed: AppConfigToml =
            toml::from_str(&content).expect("default config content should parse without errors");
        assert_eq!(parsed.provider.name, "");
        assert_eq!(parsed.provider.model_id, "");
        assert_eq!(parsed.provider.context_window, 0);
        assert_eq!(parsed.provider.max_tokens, 0);
        assert_eq!(
            parsed.directories.len(),
            1,
            "default config has the default workspace directory"
        );
        assert_eq!(parsed.directories[0].path, "~/.operon/workspace");
    }

    #[test]
    fn test_save_provider_preserves_comments() {
        let temp = tempfile::tempdir().unwrap();
        let fake_paths = OperonPaths {
            config_dir: temp.path().join(".operon"),
            workspace_dir: temp.path().join(".operon").join("workspace"),
            config_file: temp.path().join(".operon").join("config.toml"),
            sessions_dir: temp.path().join(".operon").join("sessions"),
            memory_dir: temp.path().join(".operon").join("memory"),
            memory_db: temp.path().join(".operon").join("memory").join("memory.db"),
        };

        let provider_config = ProviderConfig {
            provider: Provider::Groq,
            credentials: ApiCredentials::with_key("gsk-test-key"),
            model: ModelConfig {
                model_id: "openai/gpt-oss-120b".to_string(),
                context_window: 128_000,
                max_tokens: 4_096,
            },
            base_url_override: None,
        };

        save_provider_at_paths(&fake_paths, &provider_config)
            .expect("saving provider should succeed");

        let content =
            std::fs::read_to_string(&fake_paths.config_file).expect("config file should exist");

        assert!(
            content.contains("# PROVIDER"),
            "the saved file should keep the template comments"
        );
        assert!(
            content.contains("# GLOBAL TOOL PERMISSIONS"),
            "the saved file should keep the permission comments"
        );

        // Re-parse the file to verify the rewritten values without depending on
        // TOML editor whitespace formatting.
        let parsed: AppConfigToml =
            toml::from_str(&content).expect("saved config should still parse cleanly");
        assert_eq!(parsed.provider.name, "groq");
        assert_eq!(parsed.provider.model_id, "openai/gpt-oss-120b");
        assert_eq!(parsed.credentials.api_key, "gsk-test-key");
    }

    #[test]
    fn test_api_key_env_var_all_providers() {
        // Every provider must return either a known env var or "".
        assert_eq!(api_key_env_var("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(api_key_env_var("open_ai"), "OPENAI_API_KEY");
        assert_eq!(api_key_env_var("gemini"), "GEMINI_API_KEY");
        assert_eq!(api_key_env_var("ollama"), "");
        assert_eq!(api_key_env_var("deep_seek"), "DEEPSEEK_API_KEY");
        assert_eq!(api_key_env_var("groq"), "GROQ_API_KEY");
        assert_eq!(api_key_env_var("mistral"), "MISTRAL_API_KEY");
        assert_eq!(api_key_env_var("xai"), "XAI_API_KEY");
        assert_eq!(api_key_env_var("cohere"), "COHERE_API_KEY");
    }

    #[test]
    fn test_inject_default_workspace_when_absent() {
        // If workspace is not in the list, it must be injected.
        let paths = OperonPaths::resolve().unwrap();
        let mut dirs: Vec<DirectoryPolicy> = Vec::new();
        inject_default_workspace(&mut dirs, &paths);
        assert_eq!(dirs.len(), 1, "workspace should be injected");
        assert_eq!(dirs[0].path, paths.workspace_dir);
    }

    #[test]
    fn test_inject_default_workspace_does_not_duplicate() {
        // If workspace is already in the list, do not add a second entry.
        let paths = OperonPaths::resolve().unwrap();
        let mut dirs = vec![DirectoryPolicy::owner_full_access(
            paths.workspace_dir.clone(),
        )];
        inject_default_workspace(&mut dirs, &paths);
        assert_eq!(dirs.len(), 1, "workspace should not be duplicated");
    }

    #[test]
    fn test_load_with_temp_home() {
        // Use a temp dir as a fake home to test load() end-to-end without
        // touching the real ~/.operon directory.
        let tmp = tempfile::tempdir().unwrap();
        let fake_paths = OperonPaths {
            config_dir: tmp.path().join(".operon"),
            workspace_dir: tmp.path().join(".operon").join("workspace"),
            config_file: tmp.path().join(".operon").join("config.toml"),
            sessions_dir: tmp.path().join(".operon").join("sessions"),
            memory_dir: tmp.path().join(".operon").join("memory"),
            memory_db: tmp.path().join(".operon").join("memory").join("memory.db"),
        };

        // Create the dirs manually (normally done by ensure_dirs_exist).
        fake_paths.ensure_dirs_exist().unwrap();

        // Write a test config with Ollama (no API key needed).
        let test_config = r#"
[provider]
name           = "ollama"
model_id       = "llama3.2"
context_window = 128000
max_tokens     = 8192

[credentials]
api_key = ""

[policy.global.owner]
web = "allow"
"#;
        std::fs::write(&fake_paths.config_file, test_config).unwrap();

        // Parse the config TOML.
        let toml_text = std::fs::read_to_string(&fake_paths.config_file).unwrap();
        let toml_config: AppConfigToml = toml::from_str(&toml_text).unwrap();

        // Build provider config (Ollama — no key required).
        let provider =
            build_provider_config(&toml_config.provider, &toml_config.credentials, None).unwrap();

        assert_eq!(provider.provider, operon_providers::Provider::Ollama);
        assert_eq!(provider.model_id(), "llama3.2");

        // Build policy — inject workspace manually for this test.
        let global = build_global_policy(&toml_config.policy.global);
        let mut directories: Vec<DirectoryPolicy> = Vec::new();
        inject_default_workspace(&mut directories, &fake_paths);

        let mut policy = PolicyConfig {
            global,
            directories,
        };
        // validate() would fail if workspace doesn't exist — it does in our temp dir.
        policy.validate().unwrap();

        assert!(
            !policy.directories.is_empty(),
            "workspace should be in policy"
        );
    }

    #[test]
    fn test_add_remove_update_permissions() {
        let temp = tempfile::tempdir().unwrap();
        let fake_paths = OperonPaths {
            config_dir: temp.path().join(".operon"),
            workspace_dir: temp.path().join(".operon").join("workspace"),
            config_file: temp.path().join(".operon").join("config.toml"),
            sessions_dir: temp.path().join(".operon").join("sessions"),
            memory_dir: temp.path().join(".operon").join("memory"),
            memory_db: temp.path().join(".operon").join("memory").join("memory.db"),
        };
        fake_paths.ensure_dirs_exist().unwrap();

        // Write default config
        let default_content = default_config_content();
        std::fs::write(&fake_paths.config_file, default_content).unwrap();

        // Test add directory
        let new_dir = temp.path().join("my-cool-project");
        std::fs::create_dir_all(&new_dir).unwrap();
        let new_dir_str = new_dir.to_string_lossy().to_string();

        add_allowed_directory_at_paths(&fake_paths, &new_dir_str)
            .expect("adding allowed directory should succeed");

        // Reload to check if it's there
        let content = std::fs::read_to_string(&fake_paths.config_file).unwrap();
        assert!(
            content.contains("my-cool-project"),
            "TOML should contain new directory"
        );

        // Test update permission
        update_permission_at_paths(
            &fake_paths,
            "owner",
            Some(&new_dir_str),
            "fs_delete",
            Some("allow"),
        )
        .expect("updating permission should succeed");

        let content_after_update = std::fs::read_to_string(&fake_paths.config_file).unwrap();
        assert!(content_after_update.contains("fs_delete = \"allow\""));

        // Test remove directory
        remove_allowed_directory_at_paths(&fake_paths, &new_dir_str)
            .expect("removing allowed directory should succeed");
        let content_after_remove = std::fs::read_to_string(&fake_paths.config_file).unwrap();
        assert!(
            !content_after_remove.contains("my-cool-project"),
            "TOML should not contain new directory anymore"
        );

        // Test removing default workspace is blocked
        let remove_ws_result =
            remove_allowed_directory_at_paths(&fake_paths, "~/.operon/workspace");
        assert!(
            remove_ws_result.is_err(),
            "removing default workspace should fail"
        );
    }
}

/// Save provider configuration to `~/.operon/config.toml`.
///
/// This updates only the provider and credentials sections, preserving
/// all other configuration (policy, directories, etc.).
///
/// # What this does
///
/// 1. Loads the existing config file (or creates default if missing).
/// 2. Updates the [provider] and [credentials] sections with new values.
/// 3. Writes the modified TOML back to disk.
///
/// # Errors
///
/// Returns `ConfigError` for any I/O or serialization failure.
pub fn save_provider(
    provider_config: &operon_providers::ProviderConfig,
) -> Result<(), ConfigError> {
    // Step 1: Resolve paths.
    let paths = OperonPaths::resolve()?;

    save_provider_at_paths(&paths, provider_config)
}

/// Save provider configuration using a pre-resolved path set.
///
/// The public [`save_provider`] wrapper resolves the user's home directory and
/// forwards here. Tests use this helper directly so they can point Operon at a
/// temporary directory without relying on platform-specific HOME semantics.
fn save_provider_at_paths(
    paths: &OperonPaths,
    provider_config: &operon_providers::ProviderConfig,
) -> Result<(), ConfigError> {
    // Step 2: Ensure directories exist.
    paths.ensure_dirs_exist()?;

    // Step 3: Read existing config or create default.
    let toml_text = read_or_create_config(paths)?;

    // Step 4: Parse the existing TOML document while preserving comments.
    let mut doc = toml_text
        .parse::<DocumentMut>()
        .map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: toml::de::Error::custom(format!(
                "failed to parse existing config for update: {}",
                e
            )),
        })?;

    // Step 5: Update the provider section in place so comments survive the write.
    let provider_name = serde_json::to_string(&provider_config.provider)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();

    if doc.get("provider").is_none() {
        doc.insert("provider", Item::Table(Table::new()));
    }
    if doc.get("credentials").is_none() {
        doc.insert("credentials", Item::Table(Table::new()));
    }

    let provider = doc.get_mut("provider").unwrap().as_table_mut().unwrap();
    provider.insert("name", value(provider_name));
    provider.insert("model_id", value(provider_config.model.model_id.clone()));
    provider.insert(
        "context_window",
        value(provider_config.model.context_window as i64),
    );
    provider.insert("max_tokens", value(provider_config.model.max_tokens as i64));

    let credentials = doc.get_mut("credentials").unwrap().as_table_mut().unwrap();
    credentials.insert(
        "api_key",
        value(provider_config.credentials.api_key.expose().to_string()),
    );
    if let Some(org_id) = &provider_config.credentials.org_id {
        credentials.insert("org_id", value(org_id.clone()));
    } else {
        credentials.remove("org_id");
    }

    // Step 6: Serialize the edited document and write it back to disk.
    fs::write(&paths.config_file, doc.to_string())?;

    Ok(())
}

fn resolve_path(paths: &OperonPaths, raw_path: &str) -> std::path::PathBuf {
    let path = if raw_path.starts_with('~') {
        if let Some(home) = paths.config_dir.parent() {
            let without_tilde = raw_path
                .trim_start_matches('~')
                .trim_start_matches(['/', '\\']);
            home.join(without_tilde)
        } else {
            std::path::PathBuf::from(raw_path)
        }
    } else {
        std::path::PathBuf::from(raw_path)
    };
    if let Ok(canon) = std::fs::canonicalize(&path) {
        canon
    } else {
        path
    }
}

fn path_matches(paths: &OperonPaths, toml_path: &str, target_path: &str) -> bool {
    let p1 = resolve_path(paths, toml_path);
    let p2 = resolve_path(paths, target_path);

    p1 == p2
        || p1.to_string_lossy().trim_end_matches(['/', '\\'])
            == p2.to_string_lossy().trim_end_matches(['/', '\\'])
}

/// Add a new allowed directory to the config.toml file.
pub fn add_allowed_directory(path_str: &str) -> Result<(), ConfigError> {
    let paths = OperonPaths::resolve()?;
    add_allowed_directory_at_paths(&paths, path_str)
}

/// Add a new allowed directory using pre-resolved paths.
fn add_allowed_directory_at_paths(paths: &OperonPaths, path_str: &str) -> Result<(), ConfigError> {
    paths.ensure_dirs_exist()?;

    let toml_text = read_or_create_config(paths)?;
    let mut doc = toml_text
        .parse::<DocumentMut>()
        .map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: toml::de::Error::custom(format!(
                "failed to parse config for directory add: {}",
                e
            )),
        })?;

    if doc.get("directories").is_none() {
        doc.insert(
            "directories",
            Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
        );
    }

    let dirs = doc
        .get_mut("directories")
        .unwrap()
        .as_array_of_tables_mut()
        .unwrap();

    let already_exists = dirs.iter().any(|table| {
        if let Some(p) = table.get("path").and_then(|v| v.as_str()) {
            path_matches(paths, p, path_str)
        } else {
            false
        }
    });

    if !already_exists {
        let mut new_table = Table::new();
        new_table.insert("path", value(path_str.to_string()));

        let mut permissions = Table::new();

        let mut owner = Table::new();
        owner.insert("fs", value("ask"));
        owner.insert("bash", value("ask"));
        permissions.insert("owner", Item::Table(owner));

        let mut external = Table::new();
        external.insert("fs", value("deny"));
        external.insert("bash", value("deny"));
        permissions.insert("external", Item::Table(external));

        new_table.insert("permissions", Item::Table(permissions));
        dirs.push(new_table);

        fs::write(&paths.config_file, doc.to_string())?;
    }

    Ok(())
}

/// Remove an allowed directory from the config.toml file.
pub fn remove_allowed_directory(path_str: &str) -> Result<(), ConfigError> {
    let paths = OperonPaths::resolve()?;
    remove_allowed_directory_at_paths(&paths, path_str)
}

/// Remove an allowed directory using pre-resolved paths.
fn remove_allowed_directory_at_paths(
    paths: &OperonPaths,
    path_str: &str,
) -> Result<(), ConfigError> {
    paths.ensure_dirs_exist()?;

    let is_workspace = path_matches(paths, "~/.operon/workspace", path_str)
        || path_matches(paths, &paths.workspace_dir.display().to_string(), path_str);

    if is_workspace {
        return Err(ConfigError::PolicyValidation(
            crate::policy::PolicyError::InvalidConfig {
                reason: "The default workspace directory (~/.operon/workspace) cannot be removed."
                    .to_string(),
            },
        ));
    }

    let toml_text = read_or_create_config(paths)?;
    let mut doc = toml_text
        .parse::<DocumentMut>()
        .map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: toml::de::Error::custom(format!(
                "failed to parse config for directory remove: {}",
                e
            )),
        })?;

    if let Some(dirs) = doc
        .get_mut("directories")
        .and_then(|i| i.as_array_of_tables_mut())
    {
        let mut found_index = None;
        for (i, table) in dirs.iter().enumerate() {
            if let Some(p) = table.get("path").and_then(|v| v.as_str()) {
                if path_matches(paths, p, path_str) {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = found_index {
            dirs.remove(idx);
            fs::write(&paths.config_file, doc.to_string())?;
        }
    }

    Ok(())
}

/// Update permission mode (allow/ask/deny) for a tool/group in global or directory scope.
pub fn update_permission(
    scope: &str,
    directory: Option<&str>,
    key: &str,
    mode: Option<&str>,
) -> Result<(), ConfigError> {
    let paths = OperonPaths::resolve()?;
    update_permission_at_paths(&paths, scope, directory, key, mode)
}

/// Update permission mode using pre-resolved paths.
fn update_permission_at_paths(
    paths: &OperonPaths,
    scope: &str,
    directory: Option<&str>,
    key: &str,
    mode: Option<&str>,
) -> Result<(), ConfigError> {
    paths.ensure_dirs_exist()?;

    let toml_text = read_or_create_config(paths)?;
    let mut doc = toml_text
        .parse::<DocumentMut>()
        .map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: toml::de::Error::custom(format!(
                "failed to parse config for permission update: {}",
                e
            )),
        })?;

    if let Some(dir_path) = directory {
        if doc.get("directories").is_none() {
            doc.insert(
                "directories",
                Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
            );
        }

        let dirs = doc
            .get_mut("directories")
            .unwrap()
            .as_array_of_tables_mut()
            .unwrap();

        let mut found_index = None;
        for (i, table) in dirs.iter().enumerate() {
            if let Some(p) = table.get("path").and_then(|v| v.as_str()) {
                if path_matches(paths, p, dir_path) {
                    found_index = Some(i);
                    break;
                }
            }
        }

        let index = match found_index {
            Some(idx) => idx,
            None => {
                let mut new_table = Table::new();
                new_table.insert("path", value(dir_path.to_string()));
                dirs.push(new_table);
                dirs.len() - 1
            }
        };

        let table = dirs.get_mut(index).unwrap();

        if table.get("permissions").is_none() {
            table.insert("permissions", Item::Table(Table::new()));
        }
        let permissions = table
            .get_mut("permissions")
            .unwrap()
            .as_table_mut()
            .unwrap();

        if permissions.get(scope).is_none() {
            permissions.insert(scope, Item::Table(Table::new()));
        }
        let scope_table = permissions.get_mut(scope).unwrap().as_table_mut().unwrap();

        if let Some(mode_str) = mode {
            scope_table.insert(key, value(mode_str.to_string()));
        } else {
            scope_table.remove(key);
        }

        if scope_table.is_empty() {
            permissions.remove(scope);
        }
        if permissions.is_empty() {
            table.remove("permissions");
        }
    } else {
        if doc.get("policy").is_none() {
            doc.insert("policy", Item::Table(Table::new()));
        }
        let policy = doc.get_mut("policy").unwrap().as_table_mut().unwrap();

        if policy.get("global").is_none() {
            policy.insert("global", Item::Table(Table::new()));
        }
        let global = policy.get_mut("global").unwrap().as_table_mut().unwrap();

        if global.get(scope).is_none() {
            global.insert(scope, Item::Table(Table::new()));
        }
        let scope_table = global.get_mut(scope).unwrap().as_table_mut().unwrap();

        if let Some(mode_str) = mode {
            scope_table.insert(key, value(mode_str.to_string()));
        } else {
            scope_table.remove(key);
        }

        if scope_table.is_empty() {
            global.remove(scope);
        }
        if global.is_empty() {
            policy.remove("global");
        }
        if policy.is_empty() {
            doc.remove("policy");
        }
    }

    fs::write(&paths.config_file, doc.to_string())?;
    Ok(())
}

/// Representation of a permission row for the GUI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRow {
    pub key: String,
    pub label: String,
    pub mode: String,      // "allow", "ask", "deny", "custom"
    pub base_mode: String, // "allow", "ask", "deny"
    pub is_explicit: bool,
    pub kind: String, // "group" or "tool"
    pub group_key: String,
}

fn mode_str(mode: crate::policy::PermissionMode) -> String {
    match mode {
        crate::policy::PermissionMode::Allow => "allow".to_string(),
        crate::policy::PermissionMode::Ask => "ask".to_string(),
        crate::policy::PermissionMode::Deny => "deny".to_string(),
    }
}

/// Retrieve structured permission rows (groups/tools) from config.toml.
pub fn get_permission_rows(
    scope: &str,
    directory: Option<&str>,
) -> Result<Vec<PermissionRow>, ConfigError> {
    use crate::policy::CallerRole;

    let role = match scope {
        "owner" => CallerRole::Owner,
        "external" => CallerRole::External,
        _ => return Err(ConfigError::Internal(format!("Invalid scope: {}", scope))),
    };

    let paths = OperonPaths::resolve()?;
    // Read or create config
    let toml_text = read_or_create_config(&paths)?;
    let toml_config: AppConfigToml =
        toml::from_str(&toml_text).map_err(|e| ConfigError::TomlParse {
            path: paths.config_file.display().to_string(),
            source: e,
        })?;

    if let Some(dir_path) = directory {
        // --- Directory-scoped permissions ---
        let toml_entry = toml_config
            .directories
            .iter()
            .find(|d| path_matches(&paths, &d.path, dir_path));

        let (fs_shorthand, fs_read, fs_write, fs_edit, fs_append, fs_grep, fs_ls, fs_delete, bash) =
            if let Some(entry) = toml_entry {
                let perms = match role {
                    CallerRole::Owner => &entry.permissions.owner,
                    CallerRole::External => &entry.permissions.external,
                };
                (
                    perms.fs,
                    perms.fs_read,
                    perms.fs_write,
                    perms.fs_edit,
                    perms.fs_append,
                    perms.fs_grep,
                    perms.fs_ls,
                    perms.fs_delete,
                    perms.bash,
                )
            } else {
                // If directory is not in TOML, default to defaults
                let is_workspace = path_matches(&paths, "~/.operon/workspace", dir_path)
                    || path_matches(&paths, &paths.workspace_dir.to_string_lossy(), dir_path);

                if is_workspace && role == CallerRole::Owner {
                    (
                        Some(crate::policy::PermissionMode::Allow),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(crate::policy::PermissionMode::Allow),
                    )
                } else {
                    (
                        Some(crate::policy::PermissionMode::Deny),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(crate::policy::PermissionMode::Deny),
                    )
                }
            };

        let fs_mode = fs_shorthand.unwrap_or(crate::policy::PermissionMode::Deny);
        let fs_mode_str = mode_str(fs_mode);
        let fs_explicit = fs_shorthand.is_some();

        // Check overrides
        let has_child_overrides = [
            fs_read, fs_write, fs_edit, fs_append, fs_grep, fs_ls, fs_delete,
        ]
        .iter()
        .any(|opt| opt.is_some() && *opt != Some(fs_mode));

        let group_mode_display = if has_child_overrides {
            "custom".to_string()
        } else {
            fs_mode_str.clone()
        };

        let mut rows = Vec::new();

        // Filesystem Group
        rows.push(PermissionRow {
            key: "fs".to_string(),
            label: "File System".to_string(),
            mode: group_mode_display,
            base_mode: "deny".to_string(),
            is_explicit: fs_explicit,
            kind: "group".to_string(),
            group_key: "".to_string(),
        });

        // Filesystem Tools
        let fs_tools = [
            ("fs_read", "Read Files / Folders", fs_read),
            ("fs_write", "Create Files / Folders", fs_write),
            ("fs_edit", "Edit / Modify Files", fs_edit),
            ("fs_append", "Append to Files", fs_append),
            ("fs_grep", "Search Files (Grep)", fs_grep),
            ("fs_ls", "List Directories", fs_ls),
            ("fs_delete", "Delete Files / Folders", fs_delete),
        ];

        for (key, label, tool_opt) in fs_tools {
            rows.push(PermissionRow {
                key: key.to_string(),
                label: label.to_string(),
                mode: mode_str(tool_opt.unwrap_or(fs_mode)),
                base_mode: fs_mode_str.clone(),
                is_explicit: tool_opt.is_some(),
                kind: "tool".to_string(),
                group_key: "fs".to_string(),
            });
        }

        // Shell Execution Group
        let bash_mode = bash.unwrap_or(crate::policy::PermissionMode::Deny);
        rows.push(PermissionRow {
            key: "bash".to_string(),
            label: "Shell Execution (Bash)".to_string(),
            mode: mode_str(bash_mode),
            base_mode: "deny".to_string(),
            is_explicit: bash.is_some(),
            kind: "group".to_string(),
            group_key: "".to_string(),
        });

        Ok(rows)
    } else {
        // --- Global permissions ---
        let global_owner_map = &toml_config.policy.global.owner;
        let global_external_map = &toml_config.policy.global.external;

        let get_global_val =
            |tool: crate::policy::GlobalTool| -> (crate::policy::PermissionMode, bool) {
                let map = match role {
                    CallerRole::Owner => global_owner_map,
                    CallerRole::External => global_external_map,
                };
                if let Some(mode) = map.get(&tool) {
                    (*mode, true)
                } else {
                    (crate::policy::PermissionMode::Deny, false)
                }
            };

        let global_tools = [
            (
                crate::policy::GlobalTool::Web,
                "web",
                "Web Access (Search & Fetch)",
            ),
            (
                crate::policy::GlobalTool::SubAgent,
                "sub_agent",
                "Delegate to Sub-agents",
            ),
            (
                crate::policy::GlobalTool::Ask,
                "ask",
                "Prompt User for Input",
            ),
            (
                crate::policy::GlobalTool::Todo,
                "todo",
                "Manage Tasks / Todo Lists",
            ),
            (
                crate::policy::GlobalTool::LoadTools,
                "load_tools",
                "Load Custom Dynamic Tools",
            ),
        ];

        let mut rows = Vec::new();
        for (tool, key, label) in global_tools {
            let (mode, explicit) = get_global_val(tool);
            rows.push(PermissionRow {
                key: key.to_string(),
                label: label.to_string(),
                mode: mode_str(mode),
                base_mode: "deny".to_string(),
                is_explicit: explicit,
                kind: "group".to_string(),
                group_key: "".to_string(),
            });
        }

        Ok(rows)
    }
}

/// Retrieve the list of allowed directories and the default workspace directory, falling back to raw TOML parsing on error.
pub fn get_allowed_directories_list() -> Result<(Vec<String>, String), ConfigError> {
    let paths = OperonPaths::resolve()?;

    if let Ok(config) = load() {
        let dirs = config
            .policy
            .directories
            .iter()
            .map(|d| d.path.to_string_lossy().to_string())
            .collect();
        Ok((
            dirs,
            config.paths.workspace_dir.to_string_lossy().to_string(),
        ))
    } else {
        let config_file = &paths.config_file;
        let mut dirs = Vec::new();
        if config_file.exists() {
            if let Ok(toml_text) = std::fs::read_to_string(config_file) {
                if let Ok(parsed) = toml::from_str::<AppConfigToml>(&toml_text) {
                    dirs = parsed
                        .directories
                        .iter()
                        .map(|d| resolve_path(&paths, &d.path).to_string_lossy().to_string())
                        .collect();
                }
            }
        }

        let workspace = resolve_path(&paths, &paths.workspace_dir.to_string_lossy())
            .to_string_lossy()
            .to_string();
        if !dirs.iter().any(|d| path_matches(&paths, d, &workspace)) {
            dirs.insert(0, workspace.clone());
        }

        Ok((dirs, workspace))
    }
}
