// schema.rs — TOML deserialization schema for operon-config.
//
// This module contains:
//   1. TOML structs (AppConfigToml and its nested types) — what serde reads from disk.
//   2. AppConfig — the output type returned by load() to the rest of the system.
//   3. Conversion logic (AppConfigToml → AppConfig) via the private build functions
//      in loader.rs. The TomlX types are pub(crate) only.
//
// DESIGN: We deliberately separate the TOML schema from the output types.
// The TOML schema is user-facing (the format users type into config.toml), while
// AppConfig is code-facing (what the session runner passes around). This allows
// us to change one without breaking the other — e.g. we can rename a TOML key
// without changing any downstream consumers of AppConfig.
//
// DIRECTORY PERMISSIONS IN TOML (see DirRolePermsToml):
//
//   The flat schema supports both group-level and per-tool permissions:
//
//   [directories.permissions.owner]
//   fs     = "allow"    ← group shorthand: applies to all fs tools
//   bash   = "allow"
//
//   [directories.permissions.owner]
//   fs        = "allow"    ← base for all fs tools
//   fs_delete = "deny"     ← override for one tool only
//   bash      = "allow"
//
//   Individual keys (fs_read, fs_write, ...) take precedence over the group key (fs).
//   Missing keys default to Deny (safe by default).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use operon_providers::{ApiCredentials, ModelConfig, Provider, ProviderConfig};

use crate::error::ConfigError;
use crate::paths::OperonPaths;
use crate::policy::{
    DirTool, DirectoryPolicy, FsTool, GlobalPolicy, GlobalTool, PermissionMode, PolicyConfig,
};

// ─────────────────────────────────────────────────────────────────────────────
// TOML schema structs — pub(crate), not part of the public API
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level deserialization struct for ~/.operon/config.toml.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct AppConfigToml {
    /// Which LLM provider and model to use.
    ///
    /// These values start empty on first run so the user can choose them in the
    /// GUI instead of being forced into a hardcoded provider.
    #[serde(default)]
    pub(crate) provider: ProviderToml,

    /// API credentials. `api_key` falls back to the provider-specific env var.
    #[serde(default)]
    pub(crate) credentials: CredentialsToml,

    /// Global and directory-scoped tool permissions.
    #[serde(default)]
    pub(crate) policy: PolicyToml,

    /// Allowed directories (Direction 2 + 3). Zero or more entries.
    /// The default workspace (Direction 1) is always added by the loader,
    /// regardless of what appears here.
    #[serde(default)]
    pub(crate) directories: Vec<DirEntryToml>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// [provider] section — selects the LLM provider and model.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ProviderToml {
    /// Provider name in snake_case. Empty means "not configured yet".
    /// Valid values:
    /// anthropic, open_ai, gemini, ollama, deep_seek, open_router, groq, mistral, xai, cohere
    ///
    /// Serialized form of `operon_providers::Provider` (via serde rename_all = "snake_case").
    #[serde(default = "default_provider_name")]
    pub(crate) name: String,

    /// Exact model ID string sent in the request body.
    /// Starts empty on first run so the user can choose a model in the GUI.
    #[serde(default = "default_model_id")]
    pub(crate) model_id: String,

    /// Total token capacity of the model's context window.
    #[serde(default = "default_context_window")]
    pub(crate) context_window: usize,

    /// Maximum output tokens per turn.
    #[serde(default = "default_max_tokens")]
    pub(crate) max_tokens: usize,

    /// Optional reasoning effort / level setting for reasoning models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<String>,
}

impl Default for ProviderToml {
    fn default() -> Self {
        Self {
            name: default_provider_name(),
            model_id: default_model_id(),
            context_window: default_context_window(),
            max_tokens: default_max_tokens(),
            reasoning_effort: None,
        }
    }
}

fn default_provider_name() -> String {
    String::new()
}
fn default_model_id() -> String {
    String::new()
}
fn default_context_window() -> usize {
    200_000
}
fn default_max_tokens() -> usize {
    16_000
}

// ─────────────────────────────────────────────────────────────────────────────

/// [credentials] section — API authentication.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct CredentialsToml {
    /// Provider API key. Leave empty to load from the environment variable instead.
    ///
    /// The loader checks this field first, then falls back to the provider-specific
    /// env var (e.g. ANTHROPIC_API_KEY for Anthropic, OPENAI_API_KEY for OpenAI).
    ///
    /// For Ollama, this field is ignored — Ollama is local and auth-free.
    #[serde(default)]
    pub(crate) api_key: String,

    /// Optional organization ID. Only used by OpenAI for org-level billing.
    #[serde(default)]
    pub(crate) org_id: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// [policy] section — tool permission configuration.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct PolicyToml {
    /// [policy.global] — permissions for non-filesystem tools.
    #[serde(default)]
    pub(crate) global: GlobalPolicyToml,
}

/// [policy.global] section — permissions for web, subagent, ask, todo, load_tools.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct GlobalPolicyToml {
    /// [policy.global.owner] — tool permissions for the owner role.
    /// GlobalTool variants are serialized as snake_case (e.g. "load_tools").
    /// Missing entries default to Deny.
    #[serde(default)]
    pub(crate) owner: HashMap<GlobalTool, PermissionMode>,

    /// [policy.global.external] — tool permissions for external users.
    /// Missing entries default to Deny.
    #[serde(default)]
    pub(crate) external: HashMap<GlobalTool, PermissionMode>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// One entry in the [[directories]] array.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DirEntryToml {
    /// Path to the directory. Supports ~ expansion.
    /// Canonicalized (symlinks resolved) by PolicyConfig::validate() after loading.
    pub(crate) path: String,

    /// Tool permissions for this directory, per role.
    #[serde(default)]
    pub(crate) permissions: DirPermissionsToml,
}

/// [directories.permissions] — owner and external role permissions for a directory.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct DirPermissionsToml {
    /// [directories.permissions.owner] — what the owner can do in this directory.
    #[serde(default)]
    pub(crate) owner: DirRolePermsToml,

    /// [directories.permissions.external] — what external users can do here.
    #[serde(default)]
    pub(crate) external: DirRolePermsToml,
}

/// Flat permission table for one role inside one directory.
///
/// # Shorthand vs. individual override
///
/// `fs` is a group shorthand — it applies the same mode to every FsTool variant
/// that does not have an individual entry. Individual entries (`fs_read`, `fs_write`,
/// etc.) take precedence over the group shorthand.
///
/// Example TOML that allows all fs but denies delete:
/// ```toml
/// fs        = "allow"
/// fs_delete = "deny"
/// bash      = "allow"
/// ```
///
/// If `fs` is absent and an individual key is also absent, the tool defaults to `Deny`.
/// This enforces safe-by-default posture: unspecified tools are blocked.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct DirRolePermsToml {
    // ── Filesystem group shorthand ────────────────────────────────────────────
    /// Applies to all fs tools not overridden individually.
    /// Equivalent to setting all of fs_read/write/edit/append/grep/ls/delete.
    pub(crate) fs: Option<PermissionMode>,

    // ── Individual filesystem tool overrides ──────────────────────────────────
    // Each takes precedence over `fs` (the group shorthand) when present.
    pub(crate) fs_read: Option<PermissionMode>,
    pub(crate) fs_write: Option<PermissionMode>,
    pub(crate) fs_edit: Option<PermissionMode>,
    pub(crate) fs_append: Option<PermissionMode>,
    pub(crate) fs_grep: Option<PermissionMode>,
    pub(crate) fs_ls: Option<PermissionMode>,
    pub(crate) fs_delete: Option<PermissionMode>,

    // ── Shell ─────────────────────────────────────────────────────────────────
    /// Permission for the bash tool (shell execution) in this directory.
    pub(crate) bash: Option<PermissionMode>,
}

impl DirRolePermsToml {
    /// Converts this flat TOML struct into a `HashMap<DirTool, PermissionMode>`
    /// suitable for use in `DirectoryPolicy`.
    ///
    /// Resolution order for each tool:
    ///   1. Individual key (e.g. `fs_delete`) — highest priority.
    ///   2. Group shorthand (`fs`) — applies if no individual key is set.
    ///   3. Absent from both — the entry is omitted; resolver defaults to Deny.
    ///
    /// Only tools with an explicitly resolved mode are inserted into the map.
    /// Missing entries are intentionally absent so `PolicyResolver` can apply
    /// its own Deny default — we don't want to inflate the map with redundant denies.
    pub(crate) fn into_dir_tool_map(self) -> HashMap<DirTool, PermissionMode> {
        let mut map = HashMap::new();

        // Helper: inserts an entry if at least one source (individual or group) is present.
        let mut insert_fs = |tool: FsTool, individual: Option<PermissionMode>| {
            // Individual key wins; falls back to the group shorthand.
            if let Some(mode) = individual.or(self.fs) {
                map.insert(DirTool::Fs(tool), mode);
            }
        };

        insert_fs(FsTool::Read, self.fs_read);
        insert_fs(FsTool::Write, self.fs_write);
        insert_fs(FsTool::Edit, self.fs_edit);
        insert_fs(FsTool::Append, self.fs_append);
        insert_fs(FsTool::Grep, self.fs_grep);
        insert_fs(FsTool::Ls, self.fs_ls);
        insert_fs(FsTool::Delete, self.fs_delete);

        if let Some(bash_mode) = self.bash {
            map.insert(DirTool::Bash, bash_mode);
        }

        map
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AppConfig — the output type returned to the rest of the system
// ─────────────────────────────────────────────────────────────────────────────

/// Fully resolved, validated runtime configuration for an Operon session.
///
/// This is the single value returned by [`crate::load()`]. Callers extract
/// what they need:
///
/// - `operon-session`: takes `provider` and `policy`.
/// - `operon-context-snapshot`: takes `paths.workspace_dir` to locate AGENTS.md.
/// - `tui` / `gui`: takes `paths` for the workspace and session DB locations.
///
/// # Immutability after load
///
/// All fields are resolved, validated, and canonical at construction time.
/// There is no live-reload mechanism — restart the process to pick up config changes.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Fully assembled provider config: which provider, which model, API key.
    ///
    /// Consumed by `SessionRunner` to build HTTP requests.
    pub provider: ProviderConfig,

    /// Fully resolved policy: global tool permissions + per-directory permissions.
    ///
    /// Passed to `PolicyResolver::new()`. All directory paths are canonical.
    pub policy: PolicyConfig,

    /// All filesystem paths Operon uses at runtime.
    ///
    /// `paths.workspace_dir` is the default workspace (Direction 1).
    /// `paths.sessions_dir` is where session DBs are stored.
    pub paths: OperonPaths,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion helpers — used by loader.rs
// ─────────────────────────────────────────────────────────────────────────────

/// Parses the provider name string from TOML and returns a typed `Provider`.
///
/// The name is expected in serde snake_case (e.g. "anthropic", "open_ai").
/// Uses `serde_json` as a quick deserialization pathway for a single enum value.
///
/// # Errors
///
/// Returns `ConfigError::UnknownProvider` for names that don't match any variant.
pub(crate) fn parse_provider(name: &str) -> Result<Provider, ConfigError> {
    // Wrap the name as a JSON string and deserialize via serde.
    // This reuses the existing serde impl on Provider without a custom parser.
    let json_str = format!("\"{}\"", name);
    serde_json::from_str::<Provider>(&json_str).map_err(|_| {
        let valid = Provider::all()
            .iter()
            .map(|p| {
                // Serialize each variant to get its snake_case name.
                serde_json::to_string(p)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        ConfigError::UnknownProvider {
            name: name.to_string(),
            valid,
        }
    })
}

/// Builds a `ProviderConfig` from the TOML section + resolved credentials.
pub(crate) fn build_provider_config(
    toml: &ProviderToml,
    creds: &CredentialsToml,
    env_api_key: Option<String>,
) -> Result<ProviderConfig, ConfigError> {
    let provider_name = toml.name.trim();
    if provider_name.is_empty() {
        return Err(ConfigError::MissingProviderSelection);
    }

    let model_id = toml.model_id.trim();
    if model_id.is_empty() {
        return Err(ConfigError::MissingModelSelection {
            provider: provider_name.to_string(),
        });
    }

    let provider = parse_provider(provider_name)?;

    // API key resolution: config file wins over env var.
    // For Ollama, an empty key is fine — it's local and auth-free.
    let raw_key = if !creds.api_key.is_empty() {
        creds.api_key.clone()
    } else {
        env_api_key.unwrap_or_default()
    };

    let credentials = match creds.org_id.clone() {
        Some(org) => ApiCredentials::with_key_and_org(raw_key, org),
        None => ApiCredentials::with_key(raw_key),
    };

    let model = ModelConfig {
        model_id: model_id.to_string(),
        context_window: toml.context_window,
        max_tokens: toml.max_tokens,
        reasoning_effort: toml.reasoning_effort.clone(),
    };

    Ok(ProviderConfig {
        provider,
        credentials,
        model,
        base_url_override: None,
    })
}

/// Builds a `DirectoryPolicy` from a TOML directory entry.
///
/// Does NOT canonicalize the path — that happens in `PolicyConfig::validate()`.
pub(crate) fn build_directory_policy(entry: &DirEntryToml) -> DirectoryPolicy {
    // Expand ~ in the path string if present.
    // Full shell expansion is not supported — only leading ~.
    let raw_path = entry.path.trim();
    let path = if raw_path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            // Replace leading ~ with the home directory.
            let without_tilde = raw_path
                .trim_start_matches('~')
                .trim_start_matches(['/', '\\']);
            home.join(without_tilde)
        } else {
            PathBuf::from(raw_path)
        }
    } else {
        PathBuf::from(raw_path)
    };

    DirectoryPolicy {
        path,
        // Clone the toml structs so we can call into_dir_tool_map() which takes self.
        owner: entry.permissions.owner.clone_into_dir_tool_map(),
        external: entry.permissions.external.clone_into_dir_tool_map(),
    }
}

/// Builds a `GlobalPolicy` from the TOML global section.
pub(crate) fn build_global_policy(toml: &GlobalPolicyToml) -> GlobalPolicy {
    GlobalPolicy {
        owner: toml.owner.clone(),
        external: toml.external.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: clone-and-convert for DirRolePermsToml
// ─────────────────────────────────────────────────────────────────────────────

// DirRolePermsToml.into_dir_tool_map() consumes self. But build_directory_policy
// borrows the entry, so we need a by-reference version. The cheapest way: derive
// Clone on DirRolePermsToml. PermissionMode is Copy so clones are free.

impl Clone for DirRolePermsToml {
    fn clone(&self) -> Self {
        // All fields are Option<PermissionMode> where PermissionMode is Copy.
        Self {
            fs: self.fs,
            fs_read: self.fs_read,
            fs_write: self.fs_write,
            fs_edit: self.fs_edit,
            fs_append: self.fs_append,
            fs_grep: self.fs_grep,
            fs_ls: self.fs_ls,
            fs_delete: self.fs_delete,
            bash: self.bash,
        }
    }
}

/// Extension trait to avoid consuming self in build_directory_policy.
pub(crate) trait IntoMap {
    fn clone_into_dir_tool_map(&self) -> HashMap<DirTool, PermissionMode>;
}

impl IntoMap for DirRolePermsToml {
    fn clone_into_dir_tool_map(&self) -> HashMap<DirTool, PermissionMode> {
        self.clone().into_dir_tool_map()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_provider_known_names() {
        // These names match the serde serialized form of each Provider variant.
        // OpenAI uses explicit #[serde(rename = "open_ai")] to avoid "open_a_i".
        // XAI uses explicit #[serde(rename = "xai")] to avoid "x_a_i".
        assert_eq!(parse_provider("anthropic").unwrap(), Provider::Anthropic);
        assert_eq!(parse_provider("open_ai").unwrap(), Provider::OpenAI);
        assert_eq!(parse_provider("gemini").unwrap(), Provider::Gemini);
        assert_eq!(parse_provider("ollama").unwrap(), Provider::Ollama);
        assert_eq!(parse_provider("deep_seek").unwrap(), Provider::DeepSeek);
        assert_eq!(parse_provider("open_router").unwrap(), Provider::OpenRouter);
        assert_eq!(parse_provider("groq").unwrap(), Provider::Groq);
        assert_eq!(parse_provider("mistral").unwrap(), Provider::Mistral);
        assert_eq!(parse_provider("xai").unwrap(), Provider::XAI);
        assert_eq!(parse_provider("cohere").unwrap(), Provider::Cohere);
    }

    #[test]
    fn test_parse_provider_unknown_name() {
        let err = parse_provider("chatgpt").unwrap_err();
        assert!(err.to_string().contains("chatgpt"));
    }

    #[test]
    fn test_dir_role_perms_group_shorthand() {
        // fs = "allow" should set all 7 fs tools + still-absent bash = nothing.
        let perms = DirRolePermsToml {
            fs: Some(PermissionMode::Allow),
            ..Default::default()
        };
        let map = perms.into_dir_tool_map();
        assert_eq!(
            map.get(&DirTool::Fs(FsTool::Read)).copied(),
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            map.get(&DirTool::Fs(FsTool::Delete)).copied(),
            Some(PermissionMode::Allow)
        );
        assert!(
            !map.contains_key(&DirTool::Bash),
            "bash absent when not set"
        );
    }

    #[test]
    fn test_dir_role_perms_individual_overrides_group() {
        // fs = "allow", fs_delete = "deny" — delete should be Deny, others Allow.
        let perms = DirRolePermsToml {
            fs: Some(PermissionMode::Allow),
            fs_delete: Some(PermissionMode::Deny),
            ..Default::default()
        };
        let map = perms.into_dir_tool_map();
        assert_eq!(
            map.get(&DirTool::Fs(FsTool::Read)).copied(),
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            map.get(&DirTool::Fs(FsTool::Write)).copied(),
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            map.get(&DirTool::Fs(FsTool::Delete)).copied(),
            Some(PermissionMode::Deny)
        );
    }

    #[test]
    fn test_dir_role_perms_empty_produces_empty_map() {
        // All absent → empty map → resolver defaults all to Deny.
        let perms = DirRolePermsToml::default();
        let map = perms.into_dir_tool_map();
        assert!(map.is_empty(), "empty perms should produce empty map");
    }

    #[test]
    fn test_dir_role_perms_bash_only() {
        let perms = DirRolePermsToml {
            bash: Some(PermissionMode::Ask),
            ..Default::default()
        };
        let map = perms.into_dir_tool_map();
        assert_eq!(map.get(&DirTool::Bash).copied(), Some(PermissionMode::Ask));
        assert_eq!(map.len(), 1, "only bash should be in the map");
    }

    #[test]
    fn test_full_toml_round_trip() {
        // Parse a realistic TOML snippet to verify the schema fields.
        let toml_str = r#"
[provider]
name           = "anthropic"
model_id       = "claude-sonnet-4-20250514"
context_window = 200000
max_tokens     = 16000

[credentials]
api_key = "sk-ant-test"

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

[[directories]]
path = "/tmp/test-project"

[directories.permissions.owner]
fs   = "allow"
bash = "allow"

[directories.permissions.external]
fs   = "ask"
bash = "deny"
"#;
        let parsed: AppConfigToml = toml::from_str(toml_str).expect("should parse cleanly");
        assert_eq!(parsed.provider.name, "anthropic");
        assert_eq!(parsed.credentials.api_key, "sk-ant-test");
        assert_eq!(parsed.directories.len(), 1);
        assert_eq!(
            parsed.directories[0].permissions.owner.fs,
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            parsed.directories[0].permissions.external.bash,
            Some(PermissionMode::Deny)
        );

        // Global policy should have all owner tools set.
        let global_owner = &parsed.policy.global.owner;
        assert_eq!(
            global_owner.get(&GlobalTool::Web).copied(),
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            global_owner.get(&GlobalTool::LoadTools).copied(),
            Some(PermissionMode::Allow)
        );
    }
}
