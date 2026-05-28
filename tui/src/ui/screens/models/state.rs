// Models screen state
// Owns all state for the models configuration flow
// This includes provider selection, form inputs, and fetched model lists

use tui_textarea::TextArea;

/// Provider options available in the models screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Anthropic (Claude) - fixed base URL
    Anthropic,
    /// OpenAI (GPT) - fixed base URL
    OpenAI,
    /// Custom provider - user-configurable base URL and compatibility mode
    Custom,
}

/// Current step in the models configuration flow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelsStep {
    /// Step 1: Showing provider selection list
    ProviderList,
    /// Step 2: Showing setup form for selected provider
    Setup,
}

/// Compatibility mode for custom providers
/// Determines which API format the custom provider uses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityMode {
    /// OpenAI-compatible API (default for most local models)
    OpenAICompatible,
    /// Anthropic-compatible API
    AnthropicCompatible,
}

/// Status of the model fetch operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    /// No fetch in progress, user can press 'f' to fetch
    Idle,
    /// Currently fetching models (show spinner)
    Fetching,
    /// Fetch completed successfully, models are available
    Success,
    /// Fetch failed with error message
    #[allow(dead_code)]
    Error(String),
}

/// Complete state for the models screen
/// This is owned by AppState and contains all UI state for model configuration
pub struct ModelsState {
    /// Current step in the configuration flow (provider list or setup form)
    pub step: ModelsStep,
    
    /// Cursor position in the provider list (0-2 for Anthropic, OpenAI, Custom)
    pub selected_provider_index: usize,
    
    /// Confirmed provider selection (set when user presses Enter on provider list)
    /// None = still on provider list, Some = moved to setup form
    pub selected_provider: Option<Provider>,
    
    /// API key input field (using tui-textarea for proper text editing)
    pub api_key_input: TextArea<'static>,
    
    /// Whether API key is visible (false = masked with bullets, true = plaintext)
    /// Toggled with Tab key on the visibility toggle
    pub api_key_visible: bool,
    
    /// Custom provider base URL input field (using tui-textarea for proper text editing)
    pub base_url_input: TextArea<'static>,
    
    /// Compatibility mode for custom provider (only used for Provider::Custom)
    pub compat_mode: CompatibilityMode,
    
    /// Which form field currently has focus (0 = first field, increments with Tab)
    /// Field order varies by provider:
    /// - Anthropic/OpenAI: 0 = API key
    /// - Custom: 0 = base URL, 1 = compat mode, 2 = API key
    pub focused_field: usize,
    
    /// List of model names fetched from the provider
    /// Populated after successful mock fetch operation
    pub fetched_models: Vec<String>,
    
    /// Current status of the fetch operation
    pub fetch_status: FetchStatus,
    
    /// Cursor position in the fetched models list (for Up/Down navigation)
    pub selected_model_index: usize,
}

impl ModelsState {
    /// Create a new ModelsState with default values
    /// Starts on the provider list with Anthropic selected
    pub fn new() -> Self {
        let mut base_url_input = TextArea::default();
        base_url_input.insert_str("http://localhost:11434"); // Default Ollama URL
        
        Self {
            step: ModelsStep::ProviderList,
            selected_provider_index: 0, // Start with Anthropic selected
            selected_provider: None,
            api_key_input: TextArea::default(),
            api_key_visible: false,
            base_url_input,
            compat_mode: CompatibilityMode::OpenAICompatible,
            focused_field: 0,
            fetched_models: Vec::new(),
            fetch_status: FetchStatus::Idle,
            selected_model_index: 0,
        }
    }

    /// Move cursor up in provider list
    pub fn move_provider_up(&mut self) {
        if self.selected_provider_index > 0 {
            self.selected_provider_index -= 1;
        }
    }

    /// Move cursor down in provider list
    pub fn move_provider_down(&mut self) {
        // 3 providers: Anthropic (0), OpenAI (1), Custom (2)
        if self.selected_provider_index < 2 {
            self.selected_provider_index += 1;
        }
    }

    /// Confirm provider selection and move to setup form
    pub fn confirm_provider(&mut self) {
        self.selected_provider = Some(match self.selected_provider_index {
            0 => Provider::Anthropic,
            1 => Provider::OpenAI,
            2 => Provider::Custom,
            _ => Provider::Anthropic, // Fallback (should never happen)
        });
        self.step = ModelsStep::Setup;
        self.focused_field = 0; // Reset focus to first field
    }

    /// Go back from setup form to provider list
    pub fn back_to_provider_list(&mut self) {
        self.step = ModelsStep::ProviderList;
        self.selected_provider = None;
        // Reset form state
        self.api_key_input = TextArea::default();
        self.api_key_visible = false;
        let mut base_url_input = TextArea::default();
        base_url_input.insert_str("http://localhost:11434");
        self.base_url_input = base_url_input;
        self.compat_mode = CompatibilityMode::OpenAICompatible;
        self.focused_field = 0;
        self.fetched_models.clear();
        self.fetch_status = FetchStatus::Idle;
        self.selected_model_index = 0;
    }

    /// Move focus to next field (Tab key)
    pub fn next_field(&mut self) {
        let max_field = match self.selected_provider {
            Some(Provider::Anthropic) | Some(Provider::OpenAI) => 0, // Only API key field
            Some(Provider::Custom) => 2, // Base URL, compat mode, API key
            None => 0,
        };
        
        if self.focused_field < max_field {
            self.focused_field += 1;
        } else {
            self.focused_field = 0; // Wrap around
        }
    }

    /// Toggle API key visibility
    #[allow(dead_code)]
    pub fn toggle_api_key_visibility(&mut self) {
        self.api_key_visible = !self.api_key_visible;
    }

    /// Toggle compatibility mode (for custom provider only)
    pub fn toggle_compat_mode(&mut self) {
        self.compat_mode = match self.compat_mode {
            CompatibilityMode::OpenAICompatible => CompatibilityMode::AnthropicCompatible,
            CompatibilityMode::AnthropicCompatible => CompatibilityMode::OpenAICompatible,
        };
    }

    /// Start fetching models (sets status to Fetching)
    /// The actual async operation is handled in main.rs
    pub fn start_fetch(&mut self) {
        self.fetch_status = FetchStatus::Fetching;
        self.fetched_models.clear();
        self.selected_model_index = 0;
    }

    /// Complete fetch operation with success
    /// Called from main.rs when async fetch completes
    pub fn complete_fetch(&mut self, models: Vec<String>) {
        self.fetched_models = models;
        self.fetch_status = FetchStatus::Success;
        self.selected_model_index = 0;
    }

    /// Complete fetch operation with error
    #[allow(dead_code)]
    pub fn fail_fetch(&mut self, error: String) {
        self.fetch_status = FetchStatus::Error(error);
        self.fetched_models.clear();
    }

    /// Move cursor up in fetched models list
    pub fn move_model_up(&mut self) {
        if self.selected_model_index > 0 {
            self.selected_model_index -= 1;
        }
    }

    /// Move cursor down in fetched models list
    pub fn move_model_down(&mut self) {
        if !self.fetched_models.is_empty() && self.selected_model_index < self.fetched_models.len() - 1 {
            self.selected_model_index += 1;
        }
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        Self::new()
    }
}
