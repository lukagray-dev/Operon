// Application state module
// Owns all mutable TUI runtime state
// This is pure UI state — no business logic, no config loading
// Business state lives in operon-rs backend and is accessed via AgentBridge

pub mod screen;
pub mod session;

use screen::ActiveScreen;
use session::SessionContext;
use crate::ui::chrome::right_sidebar::panel_state::RightPanelContent;
use tui_textarea::TextArea;

/// AppState holds all mutable UI state for the TUI
/// This includes:
/// - Which screen is currently active (Chat, Models, Permissions, etc.)
/// - Whether the left sidebar is open or collapsed
/// - What content (if any) is shown in the right panel
/// - Session context data (model name, context usage, agent status)
/// - Tick counter for animations
/// - Message input TextArea widget (handles cursor, editing, etc.)
/// - Screen selector state
/// - Chat message history
/// - Chat scroll position
pub struct AppState {
    /// Currently active screen in the main panel
    active_screen: ActiveScreen,
    
    /// Whether the left sidebar (file explorer) is visible
    left_sidebar_open: bool,
    
    /// Content displayed in the right panel (None = hidden)
    right_panel: Option<RightPanelContent>,
    
    /// Session context data provided by operon-rs backend
    session: SessionContext,
    
    /// Tick counter incremented on each Action::Tick
    /// Used for animations like spinner rotation
    tick: u64,
    
    /// Message input TextArea widget for chat
    /// This handles all text editing, cursor movement, undo/redo automatically
    message_input: TextArea<'static>,
    
    /// Whether screen selector popup is open
    screen_selector_open: bool,
    
    /// Selected index in screen selector
    screen_selector_index: usize,
    
    /// Chat message history
    messages: Vec<ChatMessage>,
    
    /// Chat scroll position (0 = bottom/latest messages)
    chat_scroll: u16,
    
    /// Input scroll position (0 = bottom/latest text)
    input_scroll: u16,
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Role: "User" or "Agent"
    pub role: String,
    
    /// Message content
    pub content: String,
}

impl AppState {
    /// Create a new AppState with default values
    /// Starts on Chat screen with left sidebar open and right panel hidden
    pub fn new() -> Self {
        Self {
            active_screen: ActiveScreen::Chat,
            left_sidebar_open: true,
            right_panel: None,
            session: SessionContext::default(),
            tick: 0,
            message_input: TextArea::default(),
            screen_selector_open: false,
            screen_selector_index: 0,
            messages: Vec::new(),
            chat_scroll: 0,
            input_scroll: 0,
        }
    }

    /// Get the currently active screen
    pub fn active_screen(&self) -> &ActiveScreen {
        &self.active_screen
    }

    /// Switch to a different screen
    pub fn set_active_screen(&mut self, screen: ActiveScreen) {
        self.active_screen = screen;
    }

    /// Check if left sidebar is open
    pub fn is_left_sidebar_open(&self) -> bool {
        self.left_sidebar_open
    }

    /// Toggle left sidebar visibility
    #[allow(dead_code)]
    pub fn toggle_left_sidebar(&mut self) {
        self.left_sidebar_open = !self.left_sidebar_open;
    }

    /// Get current right panel content (None if hidden)
    pub fn right_panel(&self) -> &Option<RightPanelContent> {
        &self.right_panel
    }

    /// Set right panel content (Some = show, None = hide)
    pub fn set_right_panel(&mut self, content: Option<RightPanelContent>) {
        self.right_panel = content;
    }

    /// Get session context data
    pub fn session(&self) -> &SessionContext {
        &self.session
    }

    /// Get current tick count for animations
    #[allow(dead_code)]
    pub fn get_tick(&self) -> u64 {
        self.tick
    }

    /// Increment tick counter (called on Action::Tick)
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Get message input TextArea widget (mutable reference for rendering and input handling)
    pub fn message_input_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.message_input
    }

    /// Get message input TextArea widget (immutable reference for reading)
    pub fn message_input(&self) -> &TextArea<'static> {
        &self.message_input
    }

    /// Get the current input text as a single string
    pub fn get_input_text(&self) -> String {
        self.message_input.lines().join("\n")
    }

    /// Check if input is empty
    pub fn is_input_empty(&self) -> bool {
        self.message_input.lines().iter().all(|line| line.is_empty())
    }

    /// Clear message input and reset cursor
    pub fn clear_input(&mut self) {
        self.message_input = TextArea::default();
        // Reset input scroll when clearing
        self.input_scroll = 0;
    }

    /// Check if screen selector is open
    pub fn is_screen_selector_open(&self) -> bool {
        self.screen_selector_open
    }

    /// Open screen selector popup
    pub fn open_screen_selector(&mut self) {
        self.screen_selector_open = true;
        self.screen_selector_index = 0;
    }

    /// Close screen selector popup
    pub fn close_screen_selector(&mut self) {
        self.screen_selector_open = false;
    }

    /// Get current screen selector index
    pub fn screen_selector_index(&self) -> usize {
        self.screen_selector_index
    }

    /// Move screen selector up
    pub fn screen_selector_up(&mut self) {
        if self.screen_selector_index > 0 {
            self.screen_selector_index -= 1;
        }
    }

    /// Move screen selector down
    pub fn screen_selector_down(&mut self) {
        let max_index = ActiveScreen::all().len() - 1;
        if self.screen_selector_index < max_index {
            self.screen_selector_index += 1;
        }
    }

    /// Get selected screen from selector
    pub fn get_selected_screen(&self) -> ActiveScreen {
        ActiveScreen::all()[self.screen_selector_index]
    }

    /// Get chat message history
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Add a message to chat history
    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(ChatMessage { role, content });
        // Reset scroll to bottom when new message arrives
        self.chat_scroll = 0;
    }

    /// Get chat scroll position
    pub fn chat_scroll(&self) -> u16 {
        self.chat_scroll
    }

    /// Scroll chat up (towards older messages)
    pub fn scroll_chat_up(&mut self, amount: u16) {
        self.chat_scroll = self.chat_scroll.saturating_add(amount);
    }

    /// Scroll chat down (towards newer messages)
    pub fn scroll_chat_down(&mut self, amount: u16) {
        self.chat_scroll = self.chat_scroll.saturating_sub(amount);
    }

    /// Reset chat scroll to bottom (latest messages)
    #[allow(dead_code)]
    pub fn reset_chat_scroll(&mut self) {
        self.chat_scroll = 0;
    }

    /// Get input scroll position (deprecated - TextArea handles scrolling internally)
    #[allow(dead_code)]
    pub fn input_scroll(&self) -> u16 {
        self.input_scroll
    }

    /// Scroll input up (deprecated - TextArea handles scrolling internally)
    #[allow(dead_code)]
    pub fn scroll_input_up(&mut self, amount: u16) {
        self.input_scroll = self.input_scroll.saturating_add(amount);
    }

    /// Scroll input down (deprecated - TextArea handles scrolling internally)
    #[allow(dead_code)]
    pub fn scroll_input_down(&mut self, amount: u16) {
        self.input_scroll = self.input_scroll.saturating_sub(amount);
    }

    /// Reset input scroll to bottom (deprecated - TextArea handles scrolling internally)
    #[allow(dead_code)]
    pub fn reset_input_scroll(&mut self) {
        self.input_scroll = 0;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
