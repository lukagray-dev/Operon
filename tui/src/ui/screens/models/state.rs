// state.rs — Models screen state management for Operon TUI.
//
// DESIGN PHILOSOPHY:
// 1. Zero Business Logic:
//    - The TUI models screen is a pure frontend interface.
//    - All provider definitions, default URLs, auth requirements, and model discovery
//      are owned by `operon-rs` (`operon-providers`, `operon-config`).
// 2. Real-Time Dynamic Backend Integration:
//    - Dynamically loads all supported providers via `operon_rs::providers::Provider::all()`.
//    - Reads active configuration and saved credentials from `operon_rs::load()`.
//    - Tracks live model discovery with full token capacities and persistent config save operations.

use operon_rs::providers::{AuthHeader, DiscoveredModel, ModelConfig, Provider};
use tui_textarea::TextArea;

/// Summary information for each supported AI provider in the list view.
#[derive(Debug, Clone)]
pub struct ProviderSummaryItem {
    /// The canonical provider enum tag from operon-providers.
    pub provider: Provider,
    /// Human-friendly display label (e.g. "Anthropic", "Google Gemini", "NVIDIA NIM").
    pub label: String,
    /// Configuration status string (e.g. "Active", "Configured", "API key required").
    pub status: String,
    /// The active model identifier for this provider (if currently active).
    pub active_model: String,
    /// Whether this provider is currently the active session provider.
    pub is_active: bool,
}

/// Current navigation step in the models configuration workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelsStep {
    /// Step 1: Browse and select from all supported AI providers.
    ProviderList,
    /// Step 2: Configure endpoint, credentials, and select/discover models for the chosen provider.
    Setup,
}

/// Interactive focus targets within the provider setup form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupField {
    /// Base URL override text field.
    BaseUrl,
    /// API key / authentication token text field.
    ApiKey,
    /// "Fetch Models" auto-discovery action button.
    FetchButton,
    /// Discovered models selection list.
    DiscoveredModelList,
    /// Custom model identifier manual input field.
    CustomModel,
    /// "Save & Activate" configuration persistence button.
    SaveButton,
}

/// Real-time status of the asynchronous model discovery network request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    /// No fetch in progress; user can press [Fetch Models].
    Idle,
    /// Network request in flight (shows animated braille spinner).
    Fetching,
    /// Discovery succeeded; contains the count of discovered models.
    Success(usize),
    /// Discovery failed; contains the error explanation from operon-providers.
    Error(String),
}

/// Real-time status of the configuration save operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    /// Idle state; ready to save.
    Idle,
    /// Currently persisting to ~/.operon/config.toml.
    Saving,
    /// Successfully saved and activated provider.
    Success,
    /// Save failed; contains the error explanation.
    Error(String),
}

/// Helper to estimate/resolve a sensible context window when manually entering a model ID.
pub fn resolve_default_context_window(provider: Provider, model_id: &str) -> usize {
    let lower = model_id.to_lowercase();
    if lower.contains("gemini-1.5") || lower.contains("gemini-2.0") || lower.contains("gemini-2.5") {
        1_048_576
    } else if lower.contains("claude-3") || lower.contains("claude-sonnet") || lower.contains("claude-opus") {
        200_000
    } else if lower.contains("o1") || lower.contains("o3") || lower.contains("o4") {
        200_000
    } else if lower.contains("gpt-4") || lower.contains("gpt-3.5") {
        128_000
    } else if lower.contains("deepseek") {
        128_000
    } else if lower.contains("qwen") || lower.contains("nemotron") || lower.contains("llama-3") {
        128_000
    } else {
        match provider {
            Provider::Gemini => 1_048_576,
            Provider::Anthropic => 200_000,
            Provider::OpenAI | Provider::XAI | Provider::Groq | Provider::Mistral | Provider::NvidiaNim => 128_000,
            Provider::Ollama => 32_768,
            _ => 128_000,
        }
    }
}

/// Complete state container for the Models & Providers TUI screen.
///
/// Owned by `AppState` and updated via `Action::Models*` handlers.
pub struct ModelsState {
    /// Current step in the workflow (ProviderList or Setup).
    pub step: ModelsStep,

    /// Dynamically discovered provider summaries from operon-rs backend.
    pub providers: Vec<ProviderSummaryItem>,

    /// Cursor index in the provider selection list.
    pub selected_provider_index: usize,

    /// Currently selected provider being configured in Setup step.
    pub selected_provider: Option<Provider>,

    /// Base URL input text editor (`tui-textarea`).
    pub base_url_input: TextArea<'static>,

    /// API key input text editor (`tui-textarea`).
    pub api_key_input: TextArea<'static>,

    /// Whether the API key is displayed in plaintext or masked with bullets `••••`.
    pub api_key_visible: bool,

    /// Discovered model metadata objects returned by `operon_rs::discover_models`.
    pub discovered_models: Vec<DiscoveredModel>,

    /// Cursor index in the discovered models list.
    pub selected_model_index: usize,

    /// Manual custom model identifier text editor (`tui-textarea`).
    pub custom_model_input: TextArea<'static>,

    /// Currently focused interactive element on the setup form.
    pub focused_field: SetupField,

    /// Live status of the model discovery network request.
    pub fetch_status: FetchStatus,

    /// Live status of the config save operation.
    pub save_status: SaveStatus,
}

impl ModelsState {
    /// Constructs a new `ModelsState` and populates the initial provider list from backend config.
    pub fn new() -> Self {
        let mut state = Self {
            step: ModelsStep::ProviderList,
            providers: Vec::new(),
            selected_provider_index: 0,
            selected_provider: None,
            base_url_input: TextArea::default(),
            api_key_input: TextArea::default(),
            api_key_visible: false,
            discovered_models: Vec::new(),
            selected_model_index: 0,
            custom_model_input: TextArea::default(),
            focused_field: SetupField::ApiKey,
            fetch_status: FetchStatus::Idle,
            save_status: SaveStatus::Idle,
        };

        state.refresh_from_backend();
        state
    }

    /// Queries `operon_rs::load()` and `operon_rs::providers::Provider::all()` to build the provider list.
    pub fn refresh_from_backend(&mut self) {
        let app_config = operon_rs::load().ok();

        let active_provider = app_config.as_ref().map(|c| c.provider.provider);
        let active_model = app_config
            .as_ref()
            .map(|c| c.provider.model.model_id.clone())
            .unwrap_or_default();

        self.providers = Provider::all()
            .iter()
            .map(|&provider| {
                let is_active = active_provider.map_or(false, |p| p == provider);
                let label = provider.display_name().to_string();

                let is_configured = if let Some(ref config) = app_config {
                    config.provider.provider == provider
                        && (!config.provider.credentials.api_key.is_empty()
                            || provider == Provider::Ollama)
                } else {
                    false
                };

                let requires_key = matches!(
                    provider.capabilities().auth_header,
                    AuthHeader::Bearer | AuthHeader::XApiKey | AuthHeader::XGoogApiKey
                ) && provider != Provider::Ollama;

                let status = if is_active {
                    "Active".to_string()
                } else if is_configured {
                    "Configured".to_string()
                } else if requires_key {
                    "API key required".to_string()
                } else {
                    "Not configured".to_string()
                };

                ProviderSummaryItem {
                    provider,
                    label,
                    status,
                    active_model: if is_active {
                        active_model.clone()
                    } else {
                        String::new()
                    },
                    is_active,
                }
            })
            .collect();
    }

    /// Moves cursor up in the provider selection list.
    pub fn move_provider_up(&mut self) {
        if self.selected_provider_index > 0 {
            self.selected_provider_index -= 1;
        }
    }

    /// Moves cursor down in the provider selection list.
    pub fn move_provider_down(&mut self) {
        if !self.providers.is_empty()
            && self.selected_provider_index < self.providers.len() - 1
        {
            self.selected_provider_index += 1;
        }
    }

    /// Confirms provider selection and transitions to the Setup screen.
    pub fn confirm_provider(&mut self) {
        if self.providers.is_empty() {
            return;
        }

        let chosen = self.providers[self.selected_provider_index].provider;
        self.selected_provider = Some(chosen);
        self.load_selected_provider_details(chosen);
        self.step = ModelsStep::Setup;
    }

    /// Populates form text editors with current configuration values for the given provider.
    pub fn load_selected_provider_details(&mut self, provider: Provider) {
        let app_config = operon_rs::load().ok();
        let is_matching_active = app_config
            .as_ref()
            .map_or(false, |c| c.provider.provider == provider);

        let default_base = provider.capabilities().default_base_url;

        let (base_val, key_val, model_val) = if is_matching_active {
            if let Some(ref config) = app_config {
                let base = config
                    .provider
                    .base_url_override
                    .clone()
                    .unwrap_or_else(|| default_base.to_string());
                let key = config.provider.credentials.api_key.expose().to_string();
                let model = config.provider.model.model_id.clone();
                (base, key, model)
            } else {
                (default_base.to_string(), String::new(), String::new())
            }
        } else {
            (default_base.to_string(), String::new(), String::new())
        };

        let mut base_area = TextArea::default();
        base_area.insert_str(&base_val);
        self.base_url_input = base_area;

        let mut key_area = TextArea::default();
        key_area.insert_str(&key_val);
        self.api_key_input = key_area;

        let mut model_area = TextArea::default();
        model_area.insert_str(&model_val);
        self.custom_model_input = model_area;

        self.api_key_visible = false;
        self.discovered_models.clear();
        self.selected_model_index = 0;
        self.fetch_status = FetchStatus::Idle;
        self.save_status = SaveStatus::Idle;

        // Default focus: ApiKey for cloud providers, BaseUrl for local/custom providers like Ollama
        if provider == Provider::Ollama {
            self.focused_field = SetupField::BaseUrl;
        } else {
            self.focused_field = SetupField::ApiKey;
        }
    }

    /// Exits the setup form and returns to the provider selection list.
    pub fn back_to_provider_list(&mut self) {
        self.step = ModelsStep::ProviderList;
        self.selected_provider = None;
        self.refresh_from_backend();
    }

    /// Cycles focus to the next interactive field on the setup form.
    pub fn next_field(&mut self) {
        let has_discovered = !self.discovered_models.is_empty();

        self.focused_field = match self.focused_field {
            SetupField::BaseUrl => SetupField::ApiKey,
            SetupField::ApiKey => SetupField::FetchButton,
            SetupField::FetchButton => {
                if has_discovered {
                    SetupField::DiscoveredModelList
                } else {
                    SetupField::CustomModel
                }
            }
            SetupField::DiscoveredModelList => SetupField::CustomModel,
            SetupField::CustomModel => SetupField::SaveButton,
            SetupField::SaveButton => SetupField::BaseUrl,
        };
    }

    /// Cycles focus to the previous interactive field on the setup form.
    pub fn prev_field(&mut self) {
        let has_discovered = !self.discovered_models.is_empty();

        self.focused_field = match self.focused_field {
            SetupField::BaseUrl => SetupField::SaveButton,
            SetupField::ApiKey => SetupField::BaseUrl,
            SetupField::FetchButton => SetupField::ApiKey,
            SetupField::DiscoveredModelList => SetupField::FetchButton,
            SetupField::CustomModel => {
                if has_discovered {
                    SetupField::DiscoveredModelList
                } else {
                    SetupField::FetchButton
                }
            }
            SetupField::SaveButton => SetupField::CustomModel,
        };
    }

    /// Toggles the API key visibility between masked bullets and plaintext.
    pub fn toggle_api_key_visibility(&mut self) {
        self.api_key_visible = !self.api_key_visible;
    }

    /// Initiates a live model discovery request.
    pub fn start_fetch(&mut self) {
        self.fetch_status = FetchStatus::Fetching;
        self.discovered_models.clear();
        self.selected_model_index = 0;
    }

    /// Completes the model discovery request successfully with discovered model metadata.
    pub fn complete_fetch(&mut self, models: Vec<DiscoveredModel>) {
        let count = models.len();
        self.discovered_models = models;
        self.fetch_status = FetchStatus::Success(count);
        self.selected_model_index = 0;
        if count > 0 {
            self.focused_field = SetupField::DiscoveredModelList;
        }
    }

    /// Sets the model discovery status to error.
    pub fn fail_fetch(&mut self, error: String) {
        self.fetch_status = FetchStatus::Error(error);
        self.discovered_models.clear();
    }

    /// Moves cursor up in the discovered models list.
    pub fn move_model_up(&mut self) {
        if self.selected_model_index > 0 {
            self.selected_model_index -= 1;
        }
    }

    /// Moves cursor down in the discovered models list.
    pub fn move_model_down(&mut self) {
        if !self.discovered_models.is_empty()
            && self.selected_model_index < self.discovered_models.len() - 1
        {
            self.selected_model_index += 1;
        }
    }

    /// Copies the currently selected discovered model into the custom model input field.
    pub fn select_discovered_model(&mut self) {
        if let Some(m) = self.discovered_models.get(self.selected_model_index) {
            let mut area = TextArea::default();
            area.insert_str(&m.model_id);
            self.custom_model_input = area;
            self.focused_field = SetupField::SaveButton;
        }
    }

    /// Returns the resolved `ModelConfig` containing exact context_window and max_tokens.
    pub fn resolve_model_config(&self, provider: Provider) -> ModelConfig {
        let custom = self.custom_model_input.lines().join("").trim().to_string();

        if let Some(disc) = self.discovered_models.get(self.selected_model_index) {
            if custom.is_empty() || custom == disc.model_id {
                return ModelConfig {
                    model_id: disc.model_id.clone(),
                    context_window: disc.context_window,
                    max_tokens: disc.max_tokens,
                };
            }
        }

        let model_id = if custom.is_empty() {
            "default".to_string()
        } else {
            custom
        };

        let context_window = resolve_default_context_window(provider, &model_id);
        let max_tokens = std::cmp::min(8_192, context_window / 4);

        ModelConfig {
            model_id,
            context_window,
            max_tokens,
        }
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        Self::new()
    }
}
