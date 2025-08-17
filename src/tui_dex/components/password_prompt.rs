//! Password Prompt Modal Component
//!
//! This component provides a modal dialog for password authentication when loading saved wallets.
//! It includes password masking, error display, and retry mechanism.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Password prompt modal for secure wallet authentication
#[derive(Clone)]
pub struct PasswordPrompt {
    /// Current password input
    pub password: String,
    /// Whether password is currently visible (unmasked)
    pub password_visible: bool,
    /// Current error message if any
    pub error_message: Option<String>,
    /// Number of failed attempts
    pub failed_attempts: u32,
    /// Maximum allowed attempts before lockout
    pub max_attempts: u32,
    /// Whether the modal is currently active
    pub active: bool,
    /// Wallet name being authenticated
    pub wallet_name: String,
    /// Whether to show the cancel button
    pub show_cancel: bool,
}

impl Default for PasswordPrompt {
    fn default() -> Self {
        Self {
            password: String::new(),
            password_visible: false,
            error_message: None,
            failed_attempts: 0,
            max_attempts: 3,
            active: false,
            wallet_name: String::new(),
            show_cancel: true,
        }
    }
}

/// Result of password prompt interaction
#[derive(Debug, Clone)]
pub enum PasswordPromptResult {
    /// User entered password and confirmed
    Password(String),
    /// User cancelled the operation
    Cancelled,
    /// User exceeded maximum attempts
    TooManyAttempts,
    /// Still waiting for input
    Pending,
}

impl PasswordPrompt {
    /// Create a new password prompt for a specific wallet
    pub fn new(wallet_name: String) -> Self {
        let mut prompt = Self::default();
        prompt.wallet_name = wallet_name;
        prompt
    }

    /// Show the password prompt modal
    pub fn show(&mut self, wallet_name: String) {
        self.wallet_name = wallet_name;
        self.active = true;
        self.password.clear();
        self.error_message = None;
        // Don't reset failed attempts - they persist across sessions
    }

    /// Hide the password prompt modal
    pub fn hide(&mut self) {
        self.active = false;
        self.clear_sensitive_data();
    }

    /// Clear sensitive data from memory
    pub fn clear_sensitive_data(&mut self) {
        // Overwrite password with zeros for security
        unsafe {
            let password_bytes = self.password.as_bytes_mut();
            for byte in password_bytes {
                *byte = 0;
            }
        }
        self.password.clear();
    }

    /// Toggle password visibility
    pub fn toggle_password_visibility(&mut self) {
        self.password_visible = !self.password_visible;
    }

    /// Set error message for failed authentication
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.failed_attempts += 1;
    }

    /// Clear current error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Check if maximum attempts have been reached
    pub fn is_locked_out(&self) -> bool {
        self.failed_attempts >= self.max_attempts
    }

    /// Reset failed attempts (for admin override or after timeout)
    pub fn reset_attempts(&mut self) {
        self.failed_attempts = 0;
        self.error_message = None;
    }

    /// Handle keyboard input
    pub fn handle_key_event(&mut self, key: KeyEvent) -> PasswordPromptResult {
        if !self.active {
            return PasswordPromptResult::Pending;
        }

        // If locked out, only allow escape
        if self.is_locked_out() {
            match key.code {
                KeyCode::Esc => {
                    self.hide();
                    return PasswordPromptResult::TooManyAttempts;
                }
                _ => return PasswordPromptResult::Pending,
            }
        }

        match key.code {
            KeyCode::Char(c) => {
                self.password.push(c);
                self.clear_error(); // Clear error when user starts typing
            }
            KeyCode::Backspace => {
                self.password.pop();
                self.clear_error();
            }
            KeyCode::Enter => {
                if !self.password.is_empty() {
                    let password = self.password.clone();
                    self.hide();
                    return PasswordPromptResult::Password(password);
                }
            }
            KeyCode::Esc => {
                self.hide();
                return PasswordPromptResult::Cancelled;
            }
            KeyCode::F(1) => {
                // F1 to toggle password visibility
                self.toggle_password_visibility();
            }
            _ => {}
        }

        PasswordPromptResult::Pending
    }

    /// Get display string for password (masked or visible)
    fn get_password_display(&self) -> String {
        if self.password_visible {
            self.password.clone()
        } else {
            "•".repeat(self.password.len())
        }
    }

    /// Get status color based on current state
    fn get_status_color(&self) -> Color {
        if self.is_locked_out() {
            Color::Red
        } else if self.error_message.is_some() {
            Color::Yellow
        } else {
            Color::Blue
        }
    }
}

/// Render the password prompt modal
pub fn render_password_prompt(frame: &mut Frame, prompt: &PasswordPrompt) {
    if !prompt.active {
        return;
    }

    let size = frame.area();

    // Create centered modal area
    let modal_area = centered_rect(60, 40, size);

    // Clear the background
    frame.render_widget(Clear, modal_area);

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(2), // Wallet info
            Constraint::Length(3), // Password input
            Constraint::Min(2),    // Error message area
            Constraint::Length(2), // Help text
        ])
        .split(modal_area.inner(Margin::new(1, 1)));

    // Render modal border with appropriate color
    let border_style = Style::default()
        .fg(prompt.get_status_color())
        .add_modifier(Modifier::BOLD);

    let modal_block = Block::default()
        .borders(Borders::ALL)
        .title(" Wallet Authentication ")
        .title_alignment(Alignment::Center)
        .style(border_style);

    frame.render_widget(modal_block, modal_area);

    // Title
    let title_text = if prompt.is_locked_out() {
        Text::from(Line::from(vec![
            Span::styled("🔒 ", Style::default().fg(Color::Red)),
            Span::styled(
                "Account Locked",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]))
    } else {
        Text::from(Line::from(vec![
            Span::styled("🔑 ", Style::default().fg(Color::Blue)),
            Span::styled(
                "Enter Password",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
    };

    let title = Paragraph::new(title_text).alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Wallet info
    let wallet_info = Paragraph::new(format!("Wallet: {}", prompt.wallet_name))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(wallet_info, chunks[1]);

    // Password input
    if !prompt.is_locked_out() {
        let password_display = prompt.get_password_display();
        let password_style = if prompt.error_message.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };

        let password_block = Block::default()
            .borders(Borders::ALL)
            .title(" Password ")
            .style(password_style);

        let password_content = if password_display.is_empty() {
            "Enter password...".to_string()
        } else {
            password_display.clone()
        };

        let password_paragraph = Paragraph::new(password_content)
            .block(password_block)
            .style(if password_display.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                password_style
            });

        frame.render_widget(password_paragraph, chunks[2]);
    }

    // Error message or status
    let mut status_lines = Vec::new();

    if prompt.is_locked_out() {
        status_lines.push(Line::from(vec![Span::styled(
            "❌ Too many failed attempts!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
        status_lines.push(Line::from(vec![Span::styled(
            "Please restart the application to try again.",
            Style::default().fg(Color::Red),
        )]));
    } else if let Some(ref error) = prompt.error_message {
        status_lines.push(Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(error, Style::default().fg(Color::Red)),
        ]));

        let remaining_attempts = prompt.max_attempts - prompt.failed_attempts;
        if remaining_attempts > 0 {
            status_lines.push(Line::from(vec![Span::styled(
                format!(
                    "⚠️  {} attempt{} remaining",
                    remaining_attempts,
                    if remaining_attempts == 1 { "" } else { "s" }
                ),
                Style::default().fg(Color::Yellow),
            )]));
        }
    } else {
        status_lines.push(Line::from(vec![Span::styled(
            "Enter your wallet password to continue",
            Style::default().fg(Color::Gray),
        )]));
    }

    let status = Paragraph::new(Text::from(status_lines))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(status, chunks[3]);

    // Help text
    if !prompt.is_locked_out() {
        let help_text = if prompt.password_visible {
            "Enter: Submit • Esc: Cancel • F1: Hide Password"
        } else {
            "Enter: Submit • Esc: Cancel • F1: Show Password"
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(help, chunks[4]);
    } else {
        let help = Paragraph::new("Press Esc to close")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(help, chunks[4]);
    }
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Implement Drop for secure memory cleanup
impl Drop for PasswordPrompt {
    fn drop(&mut self) {
        self.clear_sensitive_data();
    }
}
