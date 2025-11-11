//! Password Input Component with Strength Validation
//!
//! This component provides a secure password input field with real-time
//! strength validation and visual feedback.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};
use tui_input::Input;

/// Password strength levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasswordStrength {
    Weak,
    Medium,
    Strong,
    VeryStrong,
}

impl PasswordStrength {
    /// Get color for the strength level
    pub fn color(&self) -> Color {
        match self {
            PasswordStrength::Weak => Color::Red,
            PasswordStrength::Medium => Color::Yellow,
            PasswordStrength::Strong => Color::Green,
            PasswordStrength::VeryStrong => Color::Cyan,
        }
    }

    /// Get display text for the strength level
    pub fn display(&self) -> &'static str {
        match self {
            PasswordStrength::Weak => "Weak",
            PasswordStrength::Medium => "Medium",
            PasswordStrength::Strong => "Strong",
            PasswordStrength::VeryStrong => "Very Strong",
        }
    }

    /// Get progress percentage (0-100)
    pub fn progress(&self) -> u16 {
        match self {
            PasswordStrength::Weak => 25,
            PasswordStrength::Medium => 50,
            PasswordStrength::Strong => 75,
            PasswordStrength::VeryStrong => 100,
        }
    }
}

/// Password validation result
#[derive(Debug, Clone)]
pub struct PasswordValidation {
    pub strength: PasswordStrength,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub is_valid: bool,
}

/// Password input component with validation and visual feedback
#[derive(Clone)]
pub struct PasswordInput {
    /// The input field
    input: Input,
    /// Whether password is currently visible
    is_visible: bool,
    /// Current validation result
    validation: Option<PasswordValidation>,
    /// Whether the input is focused
    is_focused: bool,
    /// Title for the input field
    title: String,
    /// Whether to show strength indicator
    show_strength: bool,
}

impl PasswordInput {
    /// Create a new password input
    pub fn new(title: String) -> Self {
        Self {
            input: Input::default(),
            is_visible: false,
            validation: None,
            is_focused: false,
            title,
            show_strength: true,
        }
    }

    /// Create a simple password input without strength indicator
    pub fn simple(title: String) -> Self {
        Self {
            input: Input::default(),
            is_visible: false,
            validation: None,
            is_focused: false,
            title,
            show_strength: false,
        }
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    /// Toggle password visibility
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }

    /// Get the password value
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Set the password value
    pub fn set_value(&mut self, value: String) {
        self.input = Input::default().with_value(value);
        self.update_validation();
    }

    /// Clear the password
    pub fn clear(&mut self) {
        self.input = Input::default();
        self.validation = None;
    }

    /// Handle character input
    pub fn handle_char(&mut self, c: char) {
        self.input.handle(tui_input::InputRequest::InsertChar(c));
        self.update_validation();
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        self.input.handle(tui_input::InputRequest::DeletePrevChar);
        self.update_validation();
    }

    /// Handle delete key
    pub fn handle_delete(&mut self) {
        self.input.handle(tui_input::InputRequest::DeleteNextChar);
        self.update_validation();
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        self.input.handle(tui_input::InputRequest::GoToPrevChar);
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        self.input.handle(tui_input::InputRequest::GoToNextChar);
    }

    /// Get current validation result
    pub fn validation(&self) -> Option<&PasswordValidation> {
        self.validation.as_ref()
    }

    /// Check if password is valid
    pub fn is_valid(&self) -> bool {
        self.validation
            .as_ref()
            .map(|v| v.is_valid)
            .unwrap_or(false)
    }

    /// Update password validation
    fn update_validation(&mut self) {
        let password = self.input.value();
        if password.is_empty() {
            self.validation = None;
            return;
        }

        let validation = Self::validate_password(password);
        self.validation = Some(validation);
    }

    /// Validate password strength and requirements
    fn validate_password(password: &str) -> PasswordValidation {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        let len = password.len();
        let has_upper = password.chars().any(|c| c.is_uppercase());
        let has_lower = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_symbol = password.chars().any(|c| !c.is_alphanumeric());

        // Check length requirement
        if len < 12 {
            issues.push(format!("Too short ({} chars)", len));
            suggestions.push("Use at least 12 characters".to_string());
        }

        // Check character requirements
        if !has_upper {
            issues.push("No uppercase letters".to_string());
            suggestions.push("Add uppercase letters (A-Z)".to_string());
        }
        if !has_lower {
            issues.push("No lowercase letters".to_string());
            suggestions.push("Add lowercase letters (a-z)".to_string());
        }
        if !has_digit {
            issues.push("No numbers".to_string());
            suggestions.push("Add numbers (0-9)".to_string());
        }
        if !has_symbol {
            issues.push("No symbols".to_string());
            suggestions.push("Add symbols (!@#$%^&*)".to_string());
        }

        // Calculate strength
        let strength =
            Self::calculate_strength(password, has_upper, has_lower, has_digit, has_symbol);
        let is_valid = issues.is_empty() && len >= 12;

        PasswordValidation {
            strength,
            issues,
            suggestions,
            is_valid,
        }
    }

    /// Calculate password strength
    fn calculate_strength(
        password: &str,
        has_upper: bool,
        has_lower: bool,
        has_digit: bool,
        has_symbol: bool,
    ) -> PasswordStrength {
        let len = password.len();
        let charset_count = [has_upper, has_lower, has_digit, has_symbol]
            .iter()
            .filter(|&&x| x)
            .count();

        // Calculate based on length and character diversity
        match (len, charset_count) {
            (0..=8, _) => PasswordStrength::Weak,
            (9..=11, 1..=2) => PasswordStrength::Weak,
            (9..=11, 3..=4) => PasswordStrength::Medium,
            (12..=15, 1..=2) => PasswordStrength::Medium,
            (12..=15, 3) => PasswordStrength::Strong,
            (12..=15, 4) => PasswordStrength::Strong,
            (16.., 3) => PasswordStrength::Strong,
            (16.., 4) => PasswordStrength::VeryStrong,
            _ => PasswordStrength::Weak,
        }
    }

    /// Render the password input field
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let chunks = if self.show_strength && self.validation.is_some() {
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Min(2),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area)
        };

        // Render input field
        let display_value = if self.is_visible {
            self.input.value().to_string()
        } else {
            "*".repeat(self.input.value().len())
        };

        let border_color = if self.is_focused {
            Color::Yellow
        } else {
            Color::Gray
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.title.as_str());

        let input_paragraph = Paragraph::new(display_value)
            .block(input_block)
            .style(Style::default().fg(Color::White));

        input_paragraph.render(chunks[0], buf);

        // Show cursor if focused
        if self.is_focused {
            let cursor_x = chunks[0].x + 1 + self.input.visual_cursor() as u16;
            let cursor_y = chunks[0].y + 1;
            if cursor_x < chunks[0].right() {
                buf[(cursor_x, cursor_y)]
                    .set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        }

        // Render strength indicator if enabled and validation exists
        if self.show_strength {
            if let Some(validation) = &self.validation {
                if chunks.len() > 2 {
                    // Render strength gauge
                    let strength_gauge = Gauge::default()
                        .block(Block::default().borders(Borders::NONE))
                        .gauge_style(Style::default().fg(validation.strength.color()))
                        .percent(validation.strength.progress())
                        .label(format!("Strength: {}", validation.strength.display()));

                    strength_gauge.render(chunks[1], buf);

                    // Render validation messages
                    let messages = if !validation.issues.is_empty() {
                        validation.issues.join(", ")
                    } else {
                        "✓ Password meets all requirements".to_string()
                    };

                    let message_color = if validation.is_valid {
                        Color::Green
                    } else {
                        Color::Red
                    };

                    let validation_paragraph = Paragraph::new(messages)
                        .style(Style::default().fg(message_color))
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    validation_paragraph.render(chunks[2], buf);
                }
            }
        }
    }

    /// Get help text for the password input
    pub fn help_text(&self) -> Vec<String> {
        let mut help = vec![
            "Password Requirements:".to_string(),
            "• At least 12 characters".to_string(),
            "• Uppercase letters (A-Z)".to_string(),
            "• Lowercase letters (a-z)".to_string(),
            "• Numbers (0-9)".to_string(),
            "• Symbols (!@#$%^&*)".to_string(),
        ];

        if self.is_visible {
            help.push("Press Ctrl+H to hide password".to_string());
        } else {
            help.push("Press Ctrl+H to show password".to_string());
        }

        help
    }
}
