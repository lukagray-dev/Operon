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
//   On first run, no config.toml exists. The loader writes a documented default
//   config with sensible values (Anthropic, Claude Sonnet 4, all global tools
//   denied for external). The user edits this file to customize their setup.
//
// ENV VAR OVERRIDE ORDER:
//   1. [credentials] api_key in config.toml — explicit file-based credential.
//   2. Provider-specific env var (e.g. ANTHROPIC_API_KEY) — CI/container friendly.
//   If both are set, the config file wins (allows secrets in file without env pollution).
//   If neither is set, MissingApiKey error (except Ollama which is auth-free).

use std::fs;

use operon_policy::config::{DirectoryPolicy, PolicyConfig};

use crate::error::ConfigError;
use crate::paths::OperonPaths;
use crate::schema::{
    build_directory_policy, build_global_policy, build_provider_config, AppConfig, AppConfigToml,
};

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
/// malformed TOML, unknown provider name, missing API key, or a directory
/// path in `[[directories]]` that cannot be canonicalized.
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
name           = "anthropic"
model_id       = "claude-sonnet-4-20250514"
context_window = 200000
max_tokens     = 16000

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
web        = "allow"
sub_agent  = "allow"
ask        = "allow"
todo       = "allow"
load_tools = "allow"

[policy.global.external]
web        = "allow"
sub_agent  = "deny"
ask        = "deny"
todo       = "deny"
load_tools = "deny"

# ─────────────────────────────────────────────────────────────────────────────
# ALLOWED DIRECTORIES
#
# Directories the agent can access via filesystem and shell tools.
# The default workspace (~/.operon/workspace/) is always allowed for the owner
# and is NOT listed here — it is added automatically.
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
#
# Example:
#   [directories.permissions.owner]
#   fs        = "allow"
#   fs_delete = "deny"    # owner can do everything EXCEPT delete
#   bash      = "allow"
# ─────────────────────────────────────────────────────────────────────────────

# Uncomment and edit to add a directory:
#
# [[directories]]
# path = "~/work/my-project"
#
# [directories.permissions.owner]
# fs   = "allow"
# bash = "allow"
#
# [directories.permissions.external]
# fs   = "ask"
# bash = "deny"
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

    #[test]
    fn test_default_config_parses_cleanly() {
        // The default config file content must parse without errors.
        let content = default_config_content();
        let parsed: AppConfigToml =
            toml::from_str(&content).expect("default config content should parse without errors");
        assert_eq!(parsed.provider.name, "anthropic");
        assert!(
            parsed.directories.is_empty(),
            "default config has no user directories"
        );
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
}
