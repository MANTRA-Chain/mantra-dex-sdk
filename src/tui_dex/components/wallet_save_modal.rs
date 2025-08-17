//! Wallet Save Modal Component
//!
//! This component provides a comprehensive modal for saving wallets with
//! encrypted password protection, form validation, and progress indicators.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph},
};
use tui_input::Input;

use super::password_input::{PasswordInput, PasswordValidation};
use crate::wallet::WalletStorage;

/// Current step in the wallet save process
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WalletSaveStep {
    WalletName,
    Password,
    PasswordConfirm,
    Confirmation,
    Saving,
    Complete,
    Error,
}

/// Wallet save operation state
#[derive(Debug, Clone)]
pub enum WalletSaveState {
    Idle,
    Validating,
    Saving {
        progress: f64,
    },
    Success {
        wallet_name: String,
    },
    Error {
        message: String,
        retry_available: bool,
    },
}

/// Form data for wallet saving
#[derive(Debug, Clone)]
pub struct WalletSaveForm {
    pub wallet_name: String,
    pub password: String,
    pub password_confirm: String,
    pub mnemonic: String,
    pub address: String,
}

impl WalletSaveForm {
    pub fn new(mnemonic: String, address: String) -> Self {
        Self {
            wallet_name: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            mnemonic,
            address,
        }
    }

    /// Validate the wallet name
    pub fn validate_wallet_name(&self) -> Result<(), String> {
        if self.wallet_name.is_empty() {
            return Err("Wallet name cannot be empty".to_string());
        }
        if self.wallet_name.len() < 3 {
            return Err("Wallet name must be at least 3 characters".to_string());
        }
        if self.wallet_name.len() > 50 {
            return Err("Wallet name must be less than 50 characters".to_string());
        }
        if !self
            .wallet_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ')
        {
            return Err(
                "Wallet name can only contain letters, numbers, spaces, hyphens, and underscores"
                    .to_string(),
            );
        }
        Ok(())
    }

    /// Validate password confirmation
    pub fn validate_password_confirmation(&self) -> Result<(), String> {
        if self.password != self.password_confirm {
            return Err("Passwords do not match".to_string());
        }
        Ok(())
    }

    /// Check if the form is valid for submission
    pub fn is_valid(&self, password_validation: Option<&PasswordValidation>) -> bool {
        self.validate_wallet_name().is_ok()
            && password_validation.map(|v| v.is_valid).unwrap_or(false)
            && self.validate_password_confirmation().is_ok()
    }
}

/// Wallet save modal component
pub struct WalletSaveModal {
    /// Whether the modal is visible
    pub is_visible: bool,
    /// Current step in the process
    pub current_step: WalletSaveStep,
    /// Form data
    pub form: WalletSaveForm,
    /// Wallet name input
    wallet_name_input: Input,
    /// Password input
    password_input: PasswordInput,
    /// Password confirmation input
    password_confirm_input: PasswordInput,
    /// Current save state
    save_state: WalletSaveState,
    /// Selected button index for navigation
    selected_button: usize,
    /// Whether to skip saving (optional)
    pub allow_skip: bool,
    /// Error message for display
    error_message: Option<String>,
    /// Validation errors
    validation_errors: Vec<String>,
}

impl WalletSaveModal {
    /// Create a new wallet save modal
    pub fn new(mnemonic: String, address: String) -> Self {
        Self {
            is_visible: false,
            current_step: WalletSaveStep::WalletName,
            form: WalletSaveForm::new(mnemonic, address),
            wallet_name_input: Input::default(),
            password_input: PasswordInput::new("Password".to_string()),
            password_confirm_input: PasswordInput::simple("Confirm Password".to_string()),
            save_state: WalletSaveState::Idle,
            selected_button: 0,
            allow_skip: true,
            error_message: None,
            validation_errors: Vec::new(),
        }
    }

    /// Show the modal
    pub fn show(&mut self) {
        self.is_visible = true;
        self.current_step = WalletSaveStep::WalletName;
        self.save_state = WalletSaveState::Idle;
        self.selected_button = 0;
        self.error_message = None;
        self.validation_errors.clear();
    }

    /// Hide the modal
    pub fn hide(&mut self) {
        self.is_visible = false;
        // Clear sensitive data
        self.form.password.clear();
        self.form.password_confirm.clear();
        self.password_input.clear();
        self.password_confirm_input.clear();
    }

    /// Get the current input focus
    pub fn focused_input(&mut self) -> Option<&mut dyn InputHandler> {
        match self.current_step {
            WalletSaveStep::WalletName => Some(&mut self.wallet_name_input),
            WalletSaveStep::Password => Some(&mut self.password_input),
            WalletSaveStep::PasswordConfirm => Some(&mut self.password_confirm_input),
            _ => None,
        }
    }

    /// Handle character input
    pub fn handle_char(&mut self, c: char) {
        self.validation_errors.clear();

        match self.current_step {
            WalletSaveStep::WalletName => {
                self.wallet_name_input
                    .handle(tui_input::InputRequest::InsertChar(c));
                self.form.wallet_name = self.wallet_name_input.value().to_string();
            }
            WalletSaveStep::Password => {
                self.password_input.handle_char(c);
                self.form.password = self.password_input.value().to_string();
            }
            WalletSaveStep::PasswordConfirm => {
                self.password_confirm_input.handle_char(c);
                self.form.password_confirm = self.password_confirm_input.value().to_string();
            }
            _ => {}
        }
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        self.validation_errors.clear();

        match self.current_step {
            WalletSaveStep::WalletName => {
                self.wallet_name_input
                    .handle(tui_input::InputRequest::DeletePrevChar);
                self.form.wallet_name = self.wallet_name_input.value().to_string();
            }
            WalletSaveStep::Password => {
                self.password_input.handle_backspace();
                self.form.password = self.password_input.value().to_string();
            }
            WalletSaveStep::PasswordConfirm => {
                self.password_confirm_input.handle_backspace();
                self.form.password_confirm = self.password_confirm_input.value().to_string();
            }
            _ => {}
        }
    }

    /// Handle tab key (password visibility toggle)
    pub fn handle_tab(&mut self) {
        match self.current_step {
            WalletSaveStep::Password => {
                self.password_input.toggle_visibility();
            }
            WalletSaveStep::PasswordConfirm => {
                self.password_confirm_input.toggle_visibility();
            }
            _ => {}
        }
    }

    /// Move to next step
    pub fn next_step(&mut self) {
        // Validate current step before proceeding
        if !self.validate_current_step() {
            return;
        }

        self.current_step = match self.current_step {
            WalletSaveStep::WalletName => WalletSaveStep::Password,
            WalletSaveStep::Password => WalletSaveStep::PasswordConfirm,
            WalletSaveStep::PasswordConfirm => WalletSaveStep::Confirmation,
            WalletSaveStep::Confirmation => {
                // Start saving process
                self.start_save_process();
                WalletSaveStep::Saving
            }
            WalletSaveStep::Complete | WalletSaveStep::Error => {
                self.hide();
                return;
            }
            _ => return,
        };

        self.selected_button = 0;
        self.validation_errors.clear();
    }

    /// Move to previous step
    pub fn previous_step(&mut self) {
        self.current_step = match self.current_step {
            WalletSaveStep::Password => WalletSaveStep::WalletName,
            WalletSaveStep::PasswordConfirm => WalletSaveStep::Password,
            WalletSaveStep::Confirmation => WalletSaveStep::PasswordConfirm,
            WalletSaveStep::Error => WalletSaveStep::Confirmation,
            _ => return,
        };

        self.selected_button = 0;
        self.validation_errors.clear();
    }

    /// Skip wallet saving
    pub fn skip_saving(&mut self) {
        if self.allow_skip {
            self.hide();
        }
    }

    /// Navigate buttons
    pub fn select_next_button(&mut self) {
        let button_count = match self.current_step {
            WalletSaveStep::Confirmation => {
                if self.allow_skip {
                    3
                } else {
                    2
                }
            }
            WalletSaveStep::Error => 2,
            _ => 2,
        };
        self.selected_button = (self.selected_button + 1) % button_count;
    }

    pub fn select_previous_button(&mut self) {
        let button_count = match self.current_step {
            WalletSaveStep::Confirmation => {
                if self.allow_skip {
                    3
                } else {
                    2
                }
            }
            WalletSaveStep::Error => 2,
            _ => 2,
        };
        self.selected_button = (self.selected_button + button_count - 1) % button_count;
    }

    /// Handle enter key
    pub fn handle_enter(&mut self) {
        match self.current_step {
            WalletSaveStep::WalletName
            | WalletSaveStep::Password
            | WalletSaveStep::PasswordConfirm => {
                if self.selected_button == 0 {
                    self.next_step();
                } else {
                    self.previous_step();
                }
            }
            WalletSaveStep::Confirmation => {
                match self.selected_button {
                    0 => self.next_step(),     // Save
                    1 => self.previous_step(), // Back
                    2 => self.skip_saving(),   // Skip (if available)
                    _ => {}
                }
            }
            WalletSaveStep::Complete => self.hide(),
            WalletSaveStep::Error => {
                if self.selected_button == 0 {
                    // Retry
                    self.current_step = WalletSaveStep::Confirmation;
                    self.selected_button = 0;
                    self.error_message = None;
                } else {
                    // Cancel
                    self.hide();
                }
            }
            _ => {}
        }
    }

    /// Validate the current step
    fn validate_current_step(&mut self) -> bool {
        self.validation_errors.clear();

        match self.current_step {
            WalletSaveStep::WalletName => {
                if let Err(e) = self.form.validate_wallet_name() {
                    self.validation_errors.push(e);
                    return false;
                }
            }
            WalletSaveStep::Password => {
                if let Some(validation) = self.password_input.validation() {
                    if !validation.is_valid {
                        self.validation_errors.extend(validation.issues.clone());
                        return false;
                    }
                } else {
                    self.validation_errors
                        .push("Password is required".to_string());
                    return false;
                }
            }
            WalletSaveStep::PasswordConfirm => {
                if let Err(e) = self.form.validate_password_confirmation() {
                    self.validation_errors.push(e);
                    return false;
                }
            }
            _ => {}
        }

        true
    }

    /// Start the wallet save process
    fn start_save_process(&mut self) {
        self.save_state = WalletSaveState::Saving { progress: 0.0 };
        // The actual saving will be handled by the parent component
    }

    /// Update save progress
    pub fn update_save_progress(&mut self, progress: f64) {
        if let WalletSaveState::Saving { .. } = self.save_state {
            self.save_state = WalletSaveState::Saving { progress };
        }
    }

    /// Complete save process
    pub fn complete_save(&mut self) {
        self.save_state = WalletSaveState::Success {
            wallet_name: self.form.wallet_name.clone(),
        };
        self.current_step = WalletSaveStep::Complete;
    }

    /// Handle save error
    pub fn handle_save_error(&mut self, error: String) {
        self.save_state = WalletSaveState::Error {
            message: error.clone(),
            retry_available: true,
        };
        self.error_message = Some(error);
        self.current_step = WalletSaveStep::Error;
    }

    /// Render the modal
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if !self.is_visible {
            return;
        }

        // Center the modal
        let modal_area = self.center_modal(area);

        // Clear the background
        Clear.render(modal_area, buf);

        // Main modal block
        let modal_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title("Save Wallet");

        // Inner area for content (calculate before rendering)
        let inner_area = modal_block.inner(modal_area);

        modal_block.render(modal_area, buf);

        match self.current_step {
            WalletSaveStep::WalletName => self.render_wallet_name_step(inner_area, buf),
            WalletSaveStep::Password => self.render_password_step(inner_area, buf),
            WalletSaveStep::PasswordConfirm => self.render_password_confirm_step(inner_area, buf),
            WalletSaveStep::Confirmation => self.render_confirmation_step(inner_area, buf),
            WalletSaveStep::Saving => self.render_saving_step(inner_area, buf),
            WalletSaveStep::Complete => self.render_complete_step(inner_area, buf),
            WalletSaveStep::Error => self.render_error_step(inner_area, buf),
        }
    }

    /// Center the modal in the given area
    fn center_modal(&self, area: Rect) -> Rect {
        let width = (area.width as f32 * 0.8).min(80.0) as u16;
        let height = match self.current_step {
            WalletSaveStep::Password => 15,
            WalletSaveStep::Confirmation => 12,
            WalletSaveStep::Saving => 8,
            _ => 10,
        };

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(area.x + x, area.y + y, width, height)
    }

    /// Render wallet name input step
    fn render_wallet_name_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Length(3), // Input
                Constraint::Length(2), // Validation errors
                Constraint::Min(2),    // Buttons
            ])
            .split(area);

        // Instructions
        let instructions = Paragraph::new("Enter a name for your wallet:")
            .style(Style::default().fg(Color::White));
        instructions.render(chunks[0], buf);

        // Input field
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title("Wallet Name");

        let input_paragraph = Paragraph::new(self.wallet_name_input.value())
            .block(input_block)
            .style(Style::default().fg(Color::White));

        input_paragraph.render(chunks[1], buf);

        // Show cursor
        let cursor_x = chunks[1].x + 1 + self.wallet_name_input.visual_cursor() as u16;
        let cursor_y = chunks[1].y + 1;
        if cursor_x < chunks[1].right() {
            buf[(cursor_x, cursor_y)].set_style(Style::default().bg(Color::White).fg(Color::Black));
        }

        // Validation errors
        self.render_validation_errors(chunks[2], buf);

        // Buttons
        self.render_step_buttons(chunks[3], buf, &["Next", "Cancel"]);
    }

    /// Render password input step
    fn render_password_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Min(6),    // Password input with strength
                Constraint::Length(2), // Validation errors
                Constraint::Length(3), // Buttons
            ])
            .split(area);

        // Instructions
        let instructions = Paragraph::new("Create a strong password to encrypt your wallet:")
            .style(Style::default().fg(Color::White));
        instructions.render(chunks[0], buf);

        // Password input
        self.password_input.set_focused(true);
        self.password_input.render(chunks[1], buf);

        // Validation errors
        self.render_validation_errors(chunks[2], buf);

        // Buttons
        self.render_step_buttons(chunks[3], buf, &["Next", "Back"]);
    }

    /// Render password confirmation step
    fn render_password_confirm_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Length(3), // Password confirm input
                Constraint::Length(2), // Validation errors
                Constraint::Min(2),    // Buttons
            ])
            .split(area);

        // Instructions
        let instructions =
            Paragraph::new("Confirm your password:").style(Style::default().fg(Color::White));
        instructions.render(chunks[0], buf);

        // Password confirmation input
        self.password_confirm_input.set_focused(true);
        self.password_confirm_input.render(chunks[1], buf);

        // Validation errors
        self.render_validation_errors(chunks[2], buf);

        // Buttons
        self.render_step_buttons(chunks[3], buf, &["Next", "Back"]);
    }

    /// Render confirmation step
    fn render_confirmation_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Min(4),    // Summary
                Constraint::Length(3), // Buttons
            ])
            .split(area);

        // Title
        let title =
            Paragraph::new("Confirm Wallet Save").style(Style::default().fg(Color::Yellow).bold());
        title.render(chunks[0], buf);

        // Summary
        let summary_text = format!(
            "Wallet Name: {}\nAddress: {}...{}\n\n⚠️  Remember your password! It cannot be recovered if lost.",
            self.form.wallet_name,
            &self.form.address[..8],
            &self.form.address[self.form.address.len()-8..]
        );

        let summary = Paragraph::new(summary_text)
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: true });
        summary.render(chunks[1], buf);

        // Buttons
        let buttons = if self.allow_skip {
            &["Save Wallet", "Back", "Skip"][..]
        } else {
            &["Save Wallet", "Back"][..]
        };
        self.render_step_buttons(chunks[2], buf, buttons);
    }

    /// Render saving progress step
    fn render_saving_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Status
                Constraint::Length(1), // Progress bar
                Constraint::Min(2),    // Progress text
            ])
            .split(area);

        // Status
        let status = Paragraph::new("Encrypting and saving wallet...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        status.render(chunks[0], buf);

        // Progress bar
        if let WalletSaveState::Saving { progress } = self.save_state {
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::NONE))
                .gauge_style(Style::default().fg(Color::Cyan))
                .percent((progress * 100.0) as u16);
            gauge.render(chunks[1], buf);
        }

        // Progress text
        let progress_text =
            Paragraph::new("Please wait while your wallet is being encrypted and saved securely.")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .wrap(ratatui::widgets::Wrap { trim: true });
        progress_text.render(chunks[2], buf);
    }

    /// Render completion step
    fn render_complete_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Success message
                Constraint::Min(3),    // Details
                Constraint::Length(3), // Button
            ])
            .split(area);

        // Success message
        let success = Paragraph::new("✓ Wallet Saved Successfully!")
            .style(Style::default().fg(Color::Green).bold())
            .alignment(Alignment::Center);
        success.render(chunks[0], buf);

        // Details
        let details_text = format!(
            "Your wallet '{}' has been encrypted and saved securely.\n\nYou can now access it from the wallet selection screen on startup.",
            self.form.wallet_name
        );

        let details = Paragraph::new(details_text)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });
        details.render(chunks[1], buf);

        // Button
        self.render_step_buttons(chunks[2], buf, &["Continue"]);
    }

    /// Render error step
    fn render_error_step(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Error message
                Constraint::Min(3),    // Error details
                Constraint::Length(3), // Buttons
            ])
            .split(area);

        // Error message
        let error = Paragraph::new("❌ Save Failed")
            .style(Style::default().fg(Color::Red).bold())
            .alignment(Alignment::Center);
        error.render(chunks[0], buf);

        // Error details
        let error_text = self
            .error_message
            .as_deref()
            .unwrap_or("An unknown error occurred while saving the wallet.");
        let details = Paragraph::new(error_text)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });
        details.render(chunks[1], buf);

        // Buttons
        self.render_step_buttons(chunks[2], buf, &["Retry", "Cancel"]);
    }

    /// Render validation errors
    fn render_validation_errors(&self, area: Rect, buf: &mut Buffer) {
        if !self.validation_errors.is_empty() {
            let error_text = self.validation_errors.join(", ");
            let error_paragraph = Paragraph::new(error_text)
                .style(Style::default().fg(Color::Red))
                .wrap(ratatui::widgets::Wrap { trim: true });
            error_paragraph.render(area, buf);
        }
    }

    /// Render step navigation buttons
    fn render_step_buttons(&self, area: Rect, buf: &mut Buffer, button_labels: &[&str]) {
        let button_width = area.width / button_labels.len() as u16;

        for (i, &label) in button_labels.iter().enumerate() {
            let x = area.x + (i as u16 * button_width);
            let button_area = Rect::new(x, area.y, button_width, area.height);

            let style = if i == self.selected_button {
                Style::default().bg(Color::Yellow).fg(Color::Black)
            } else {
                Style::default().fg(Color::White)
            };

            let button = Paragraph::new(format!(" {} ", label))
                .style(style)
                .alignment(Alignment::Center);
            button.render(button_area, buf);
        }
    }
}

/// Trait for unified input handling
pub trait InputHandler {
    fn handle_char(&mut self, c: char);
    fn handle_backspace(&mut self);
    fn value(&self) -> &str;
}

impl InputHandler for Input {
    fn handle_char(&mut self, c: char) {
        self.handle(tui_input::InputRequest::InsertChar(c));
    }

    fn handle_backspace(&mut self) {
        self.handle(tui_input::InputRequest::DeletePrevChar);
    }

    fn value(&self) -> &str {
        self.value()
    }
}

impl InputHandler for PasswordInput {
    fn handle_char(&mut self, c: char) {
        self.handle_char(c);
    }

    fn handle_backspace(&mut self) {
        self.handle_backspace();
    }

    fn value(&self) -> &str {
        self.value()
    }
}
