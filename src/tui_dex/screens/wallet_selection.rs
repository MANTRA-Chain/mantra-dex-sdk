//! Wallet Selection Screen
//!
//! This screen is shown on startup when saved wallets exist. Users can select
//! an existing wallet, create a new one, or recover from mnemonic.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::tui_dex::components::password_input::PasswordInput;
use crate::wallet::{WalletMetadata, WalletStorage};
use crate::Error;

/// Current state of the wallet selection screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WalletSelectionState {
    /// Selecting from available wallets
    SelectingWallet,
    /// Entering password for selected wallet
    EnteringPassword,
    /// Creating new wallet (goes to wizard)
    CreatingWallet,
    /// Recovering wallet (goes to wizard)
    RecoveringWallet,
    /// Loading wallet in progress
    Loading,
    /// Error state
    Error,
}

/// Wallet selection screen component
#[derive(Clone)]
pub struct WalletSelectionScreen {
    /// Current state
    pub state: WalletSelectionState,
    /// Available wallets
    pub available_wallets: Vec<WalletMetadata>,
    /// Currently selected wallet index
    pub selected_index: usize,
    /// Password input for authentication
    password_input: PasswordInput,
    /// Selected wallet for authentication
    pub selected_wallet: Option<WalletMetadata>,
    /// Error message if any
    pub error_message: Option<String>,
    /// Failed attempt count
    pub failed_attempts: u32,
    /// Maximum allowed attempts
    pub max_attempts: u32,
    /// Whether authentication is locked out
    pub is_locked_out: bool,
    /// Lockout time remaining (seconds)
    pub lockout_time_remaining: u32,
}

impl Default for WalletSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl WalletSelectionScreen {
    /// Create a new wallet selection screen
    pub fn new() -> Self {
        Self {
            state: WalletSelectionState::SelectingWallet,
            available_wallets: Vec::new(),
            selected_index: 0,
            password_input: PasswordInput::simple("Password".to_string()),
            selected_wallet: None,
            error_message: None,
            failed_attempts: 0,
            max_attempts: 3,
            is_locked_out: false,
            lockout_time_remaining: 0,
        }
    }

    /// Initialize the screen with available wallets
    pub fn initialize(&mut self) -> Result<(), Error> {
        let storage = WalletStorage::new()?;
        self.available_wallets = storage.list_wallets()?;

        if self.available_wallets.is_empty() {
            self.state = WalletSelectionState::CreatingWallet;
        } else {
            self.state = WalletSelectionState::SelectingWallet;
            self.selected_index = 0;
        }

        Ok(())
    }

    /// Move selection up
    pub fn move_selection_up(&mut self) {
        if self.state == WalletSelectionState::SelectingWallet {
            let total_options = self.available_wallets.len() + 2; // +2 for Create/Recover options
            if self.selected_index == 0 {
                self.selected_index = total_options - 1; // Wrap to last option
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Move selection down
    pub fn move_selection_down(&mut self) {
        if self.state == WalletSelectionState::SelectingWallet {
            let total_options = self.available_wallets.len() + 2; // +2 for Create/Recover options
            self.selected_index = (self.selected_index + 1) % total_options;
        }
    }

    /// Handle selection (Enter key)
    pub fn handle_selection(&mut self) -> WalletSelectionAction {
        match self.state {
            WalletSelectionState::SelectingWallet => {
                if self.selected_index < self.available_wallets.len() {
                    // Selected an existing wallet - prompt for password
                    self.selected_wallet =
                        Some(self.available_wallets[self.selected_index].clone());
                    self.state = WalletSelectionState::EnteringPassword;
                    self.password_input.set_focused(true);
                    WalletSelectionAction::None
                } else if self.selected_index == self.available_wallets.len() {
                    // Create new wallet
                    self.state = WalletSelectionState::CreatingWallet;
                    WalletSelectionAction::CreateNewWallet
                } else {
                    // Recover wallet
                    self.state = WalletSelectionState::RecoveringWallet;
                    WalletSelectionAction::RecoverWallet
                }
            }
            WalletSelectionState::EnteringPassword => {
                // Attempt to authenticate with entered password
                if let Some(wallet) = &self.selected_wallet {
                    if !self.password_input.value().is_empty() {
                        self.attempt_authentication(
                            wallet.name.clone(),
                            self.password_input.value().to_string(),
                        )
                    } else {
                        WalletSelectionAction::None
                    }
                } else {
                    WalletSelectionAction::None
                }
            }
            _ => WalletSelectionAction::None,
        }
    }

    /// Attempt to authenticate with the given password
    fn attempt_authentication(
        &mut self,
        wallet_name: String,
        password: String,
    ) -> WalletSelectionAction {
        if self.is_locked_out {
            self.error_message = Some(format!(
                "Authentication locked. Try again in {} seconds.",
                self.lockout_time_remaining
            ));
            return WalletSelectionAction::None;
        }

        self.state = WalletSelectionState::Loading;

        // Clear password from input immediately for security
        self.password_input.clear();

        WalletSelectionAction::AuthenticateWallet {
            wallet_name,
            password,
        }
    }

    /// Handle authentication success
    pub fn handle_authentication_success(
        &mut self,
        wallet_name: String,
        mnemonic: String,
    ) -> WalletSelectionAction {
        self.failed_attempts = 0;
        self.error_message = None;
        self.is_locked_out = false;
        WalletSelectionAction::WalletLoaded {
            wallet_name,
            mnemonic,
        }
    }

    /// Handle authentication failure
    pub fn handle_authentication_failure(&mut self, error: String) {
        self.failed_attempts += 1;
        self.error_message = Some(error);

        if self.failed_attempts >= self.max_attempts {
            self.is_locked_out = true;
            self.lockout_time_remaining = 300; // 5 minutes
            self.state = WalletSelectionState::Error;
        } else {
            self.state = WalletSelectionState::EnteringPassword;
            self.password_input.set_focused(true);
        }
    }

    /// Handle character input for password
    pub fn handle_char(&mut self, c: char) {
        if self.state == WalletSelectionState::EnteringPassword {
            self.password_input.handle_char(c);
            // Clear error message when user starts typing
            if self.error_message.is_some() {
                self.error_message = None;
            }
        }
    }

    /// Handle backspace for password
    pub fn handle_backspace(&mut self) {
        if self.state == WalletSelectionState::EnteringPassword {
            self.password_input.handle_backspace();
        }
    }

    /// Handle escape key
    pub fn handle_escape(&mut self) -> WalletSelectionAction {
        match self.state {
            WalletSelectionState::EnteringPassword => {
                // Return to wallet selection
                self.state = WalletSelectionState::SelectingWallet;
                self.selected_wallet = None;
                self.password_input.clear();
                self.password_input.set_focused(false);
                self.error_message = None;
                WalletSelectionAction::None
            }
            WalletSelectionState::Error => {
                // Return to wallet selection if not locked out
                if !self.is_locked_out {
                    self.state = WalletSelectionState::SelectingWallet;
                    self.error_message = None;
                }
                WalletSelectionAction::None
            }
            _ => WalletSelectionAction::Quit,
        }
    }

    /// Toggle password visibility
    pub fn toggle_password_visibility(&mut self) {
        if self.state == WalletSelectionState::EnteringPassword {
            self.password_input.toggle_visibility();
        }
    }

    /// Update lockout timer (call this periodically)
    pub fn update_lockout_timer(&mut self) {
        if self.is_locked_out && self.lockout_time_remaining > 0 {
            self.lockout_time_remaining -= 1;
            if self.lockout_time_remaining == 0 {
                self.is_locked_out = false;
                self.failed_attempts = 0;
                self.state = WalletSelectionState::SelectingWallet;
                self.error_message = None;
            }
        }
    }

    /// Render the wallet selection screen
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        match self.state {
            WalletSelectionState::SelectingWallet => self.render_wallet_list(area, buf),
            WalletSelectionState::EnteringPassword => self.render_password_prompt(area, buf),
            WalletSelectionState::Loading => self.render_loading(area, buf),
            WalletSelectionState::Error => self.render_error(area, buf),
            _ => {} // Other states handled by parent
        }
    }

    /// Render the wallet list
    fn render_wallet_list(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(5),    // Wallet list
                Constraint::Length(4), // Help text
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Select Wallet")
            .style(Style::default().fg(Color::Yellow).bold())
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        title.render(chunks[0], buf);

        // Wallet list
        let mut items = Vec::new();

        // Add existing wallets
        for (i, wallet) in self.available_wallets.iter().enumerate() {
            let style = if i == self.selected_index {
                Style::default().bg(Color::Yellow).fg(Color::Black)
            } else {
                Style::default().fg(Color::White)
            };

            let last_accessed = wallet.last_accessed.as_deref().unwrap_or("Never");

            let item = ListItem::new(format!(
                "📱 {} ({}...{}) - Last: {}",
                wallet.name,
                &wallet.address[..8],
                &wallet.address[wallet.address.len() - 8..],
                last_accessed
            ))
            .style(style);
            items.push(item);
        }

        // Add "Create New Wallet" option
        let create_style = if self.selected_index == self.available_wallets.len() {
            Style::default().bg(Color::Green).fg(Color::Black)
        } else {
            Style::default().fg(Color::Green)
        };
        items.push(ListItem::new("➕ Create New Wallet").style(create_style));

        // Add "Recover Wallet" option
        let recover_style = if self.selected_index == self.available_wallets.len() + 1 {
            Style::default().bg(Color::Blue).fg(Color::Black)
        } else {
            Style::default().fg(Color::Blue)
        };
        items.push(ListItem::new("🔄 Recover Existing Wallet").style(recover_style));

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Wallets"),
        );
        Widget::render(list, chunks[1], buf);

        // Help text
        let help_text = "Use ↑/↓ to navigate, Enter to select, Esc to quit";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Help"));
        help.render(chunks[2], buf);
    }

    /// Render password prompt
    fn render_password_prompt(&mut self, area: Rect, buf: &mut Buffer) {
        // Center the modal
        let modal_area = self.center_modal(area, 60, 12);

        // Clear background
        Clear.render(modal_area, buf);

        // Main block
        let main_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title("Enter Password");

        let inner_area = main_block.inner(modal_area);
        main_block.render(modal_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Wallet info
                Constraint::Length(3), // Password input
                Constraint::Length(2), // Error message
                Constraint::Length(2), // Help text
            ])
            .split(inner_area);

        // Wallet info
        if let Some(wallet) = &self.selected_wallet {
            let info = Paragraph::new(format!("Wallet: {}", wallet.name))
                .style(Style::default().fg(Color::White));
            info.render(chunks[0], buf);
        }

        // Password input
        self.password_input.render(chunks[1], buf);

        // Error message
        if let Some(error) = &self.error_message {
            let error_text = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .wrap(ratatui::widgets::Wrap { trim: true });
            error_text.render(chunks[2], buf);
        }

        // Help text
        let help_text = if self.failed_attempts > 0 {
            format!(
                "Enter password (Attempt {}/{}) • Ctrl+H to toggle visibility • Esc to cancel",
                self.failed_attempts + 1,
                self.max_attempts
            )
        } else {
            "Enter password • Ctrl+H to toggle visibility • Esc to cancel".to_string()
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .wrap(ratatui::widgets::Wrap { trim: true });
        help.render(chunks[3], buf);
    }

    /// Render loading state
    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        let modal_area = self.center_modal(area, 40, 6);
        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("Loading");

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let loading_text = "Decrypting wallet...\nPlease wait...";
        let paragraph = Paragraph::new(loading_text)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        paragraph.render(inner_area, buf);
    }

    /// Render error state
    fn render_error(&self, area: Rect, buf: &mut Buffer) {
        let modal_area = self.center_modal(area, 50, 8);
        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title("Authentication Failed");

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let error_text = if self.is_locked_out {
            format!(
                "Too many failed attempts!\n\nAuthentication locked for {} seconds.\nPlease wait before trying again.",
                self.lockout_time_remaining
            )
        } else if let Some(error) = &self.error_message {
            error.clone()
        } else {
            "An unknown error occurred.".to_string()
        };

        let paragraph = Paragraph::new(error_text)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });
        paragraph.render(inner_area, buf);
    }

    /// Center a modal in the given area
    fn center_modal(&self, area: Rect, width: u16, height: u16) -> Rect {
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        Rect::new(area.x + x, area.y + y, width, height)
    }
}

/// Actions that can be returned from wallet selection screen
#[derive(Debug, Clone)]
pub enum WalletSelectionAction {
    /// No action needed
    None,
    /// User wants to create a new wallet
    CreateNewWallet,
    /// User wants to recover a wallet
    RecoverWallet,
    /// Attempt to authenticate with the given wallet
    AuthenticateWallet {
        wallet_name: String,
        password: String,
    },
    /// Wallet was successfully loaded
    WalletLoaded {
        wallet_name: String,
        mnemonic: String,
    },
    /// User wants to quit the application
    Quit,
}

/// Render the wallet selection screen (Frame-based interface for UI module)
pub fn render_wallet_selection(frame: &mut ratatui::Frame, app: &crate::tui_dex::app::App) {
    let area = frame.area();
    let mut buf = frame.buffer_mut();

    // Access the wallet selection state and call its render method
    let mut wallet_selection_state = app.state.wallet_selection_state.clone();
    wallet_selection_state.render(area, buf);
}
