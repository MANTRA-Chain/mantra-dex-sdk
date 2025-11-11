//! Form Input Components for MANTRA DEX TUI
//!
//! This module provides reusable form input components including text inputs
//! with validation, dropdowns for selections, and checkboxes for toggles.
//!
//! # Example Usage
//!
//! ```rust
//! use mantra_dex_sdk::tui::components::forms::*;
//! use tui_input::InputRequest;
//!
//! // Create a text input for wallet addresses
//! let mut address_input = TextInput::new("Wallet Address")
//!     .with_type(InputType::Address)
//!     .required()
//!     .with_placeholder("mantra1...");
//!
//! // Create a dropdown for pool selection
//! let mut pool_dropdown = Dropdown::new("Select Pool")
//!     .add_option(DropdownOption::new("USDC/USDT Pool", 1))
//!     .add_option(DropdownOption::new("ATOM/OSMO Pool", 2))
//!     .required();
//!
//! // Create a checkbox for feature toggle
//! let mut slippage_checkbox = Checkbox::new("Enable Slippage Protection");
//!
//! // Handle user input
//! address_input.handle_input(InputRequest::InsertChar('m'));
//! address_input.validate(); // Returns true/false
//!
//! // Check if inputs are valid
//! if address_input.is_valid() && pool_dropdown.selected_value().is_some() {
//!     // Proceed with transaction
//! }
//! ```

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use tui_input::{Input, InputRequest, InputResponse};

/// Text input component with validation for addresses, amounts, and pool IDs
#[derive(Debug, Clone)]
pub struct TextInput {
    /// The underlying tui-input component
    input: Input,
    /// Label for the input field
    label: String,
    /// Whether the input is focused
    focused: bool,
    /// Validation error message
    error: Option<String>,
    /// Input type for validation
    input_type: InputType,
    /// Whether the input is required
    required: bool,
    /// Placeholder text
    placeholder: String,
}

/// Input validation types
#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    /// Any text input
    Text,
    /// Cryptocurrency address validation
    Address,
    /// Numeric amount with decimal support
    Amount,
    /// Pool ID validation (typically numeric)
    PoolId,
    /// Email address validation
    Email,
    /// Password input (hidden text)
    Password,
}

impl TextInput {
    /// Create a new text input with a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            input: Input::default(),
            label: label.into(),
            focused: false,
            error: None,
            input_type: InputType::Text,
            required: false,
            placeholder: String::new(),
        }
    }

    /// Set the input type for validation
    pub fn with_type(mut self, input_type: InputType) -> Self {
        self.input_type = input_type;
        self
    }

    /// Mark the input as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set placeholder text
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the current value
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.input = Input::default().with_value(value.into());
        self
    }

    /// Set the current value of the input field.
    pub fn set_value(&mut self, value: &str) {
        self.input = self.input.clone().with_value(value.to_string());
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if this checkbox is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Clear the input
    pub fn clear(&mut self) {
        self.input = Input::default();
        self.error = None;
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, request: InputRequest) -> InputResponse {
        let response = self.input.handle(request);
        // Clear error when user starts typing
        if matches!(request, InputRequest::InsertChar(_)) {
            self.error = None;
        }
        self.validate();
        response
    }

    /// Validate the current input
    pub fn validate(&mut self) -> bool {
        self.error = None;

        let value = self.input.value().trim().to_string();

        // Check if required field is empty
        if self.required && value.is_empty() {
            self.error = Some("This field is required".to_string());
            return false;
        }

        // Skip validation for empty optional fields
        if value.is_empty() {
            return true;
        }

        match self.input_type {
            InputType::Text => true,
            InputType::Address => self.validate_address(&value),
            InputType::Amount => self.validate_amount(&value),
            InputType::PoolId => self.validate_pool_id(&value),
            InputType::Email => self.validate_email(&value),
            InputType::Password => self.validate_password(&value),
        }
    }

    /// Get validation error if any
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Check if input is valid
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }

    fn validate_address(&mut self, value: &str) -> bool {
        // Basic address validation (mantra addresses start with "mantra")
        if value.starts_with("mantra") && value.len() >= 40 {
            true
        } else {
            self.error = Some("Invalid address format (should start with 'mantra')".to_string());
            false
        }
    }

    fn validate_amount(&mut self, value: &str) -> bool {
        match value.parse::<f64>() {
            Ok(amount) if amount >= 0.0 => true,
            Ok(_) => {
                self.error = Some("Amount must be positive".to_string());
                false
            }
            Err(_) => {
                self.error = Some("Invalid amount format".to_string());
                false
            }
        }
    }

    fn validate_pool_id(&mut self, value: &str) -> bool {
        match value.parse::<u64>() {
            Ok(_) => true,
            Err(_) => {
                self.error = Some("Pool ID must be a number".to_string());
                false
            }
        }
    }

    fn validate_email(&mut self, value: &str) -> bool {
        if value.contains('@') && value.contains('.') {
            true
        } else {
            self.error = Some("Invalid email format".to_string());
            false
        }
    }

    fn validate_password(&mut self, value: &str) -> bool {
        if value.len() >= 8 {
            true
        } else {
            self.error = Some("Password must be at least 8 characters".to_string());
            false
        }
    }

    /// Render the text input
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Label
                Constraint::Length(3), // Input box (fixed height for single line)
                Constraint::Length(1), // Error message
            ])
            .split(area);

        // Render label with better visibility
        let label_style = if self.required {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightBlue)
        };

        let label_text = if self.required {
            format!("{} *", self.label)
        } else {
            self.label.clone()
        };

        frame.render_widget(Paragraph::new(label_text).style(label_style), chunks[0]);

        // Render input box with improved styling
        let border_style = if self.focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if self.error.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Blue)
        };

        let block = Block::default().borders(Borders::ALL).style(border_style);

        let display_value = if self.input_type == InputType::Password {
            "*".repeat(self.input.value().len())
        } else if self.input.value().is_empty() && !self.focused {
            self.placeholder.clone()
        } else {
            self.input.value().to_string()
        };

        // Improved text styling for better readability
        let text_style = if self.input.value().is_empty() && !self.focused {
            // Placeholder text - more visible
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC)
        } else if self.focused {
            // Focused text - high contrast white on dark background
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            // Normal text - bright white for good contrast
            Style::default().fg(Color::White)
        };

        let input_widget = Paragraph::new(display_value).block(block).style(text_style);

        frame.render_widget(input_widget, chunks[1]);

        // Show cursor if focused
        if self.focused {
            let cursor_x = chunks[1].x + self.input.visual_cursor() as u16 + 1;
            let cursor_y = chunks[1].y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        // Render error message with better visibility
        if let Some(error) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("⚠ {}", error))
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                chunks[2],
            );
        }
    }
}

/// Dropdown/Select component for pool and token selection
#[derive(Debug, Clone)]
pub struct Dropdown<T> {
    /// Label for the dropdown
    label: String,
    /// Available options
    options: Vec<DropdownOption<T>>,
    /// Currently selected option index
    selected: Option<usize>,
    /// Whether the dropdown is open
    open: bool,
    /// Whether the dropdown is focused
    focused: bool,
    /// List state for scrolling
    list_state: ListState,
    /// Whether selection is required
    required: bool,
    /// Currently highlighted option when open
    highlighted: Option<usize>,
}

/// Option for dropdown component
#[derive(Debug, Clone)]
pub struct DropdownOption<T> {
    /// Display text for the option
    pub text: String,
    /// The actual value
    pub value: T,
    /// Whether this option is enabled
    pub enabled: bool,
}

impl<T> DropdownOption<T> {
    /// Create a new dropdown option
    pub fn new(text: impl Into<String>, value: T) -> Self {
        Self {
            text: text.into(),
            value,
            enabled: true,
        }
    }

    /// Create a disabled option
    pub fn disabled(text: impl Into<String>, value: T) -> Self {
        Self {
            text: text.into(),
            value,
            enabled: false,
        }
    }
}

impl<T: Clone> Dropdown<T> {
    /// Create a new dropdown with a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            options: Vec::new(),
            selected: None,
            open: false,
            focused: false,
            list_state: ListState::default(),
            required: false,
            highlighted: None,
        }
    }

    /// Add an option to the dropdown
    pub fn add_option(mut self, option: DropdownOption<T>) -> Self {
        self.options.push(option);
        self
    }

    /// Add multiple options
    pub fn with_options(mut self, options: Vec<DropdownOption<T>>) -> Self {
        self.options = options;
        self
    }

    /// Mark as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.open = false;
            self.highlighted = None;
        }
    }

    /// Check if this dropdown is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Check if this dropdown is open
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Toggle dropdown open/closed
    pub fn toggle(&mut self) {
        if self.focused && !self.options.is_empty() {
            self.open = !self.open;
            if self.open {
                // Set initial highlight to selected item or first item
                let initial_highlight = self.selected.unwrap_or(0);
                self.highlighted = Some(initial_highlight);
                self.list_state.select(Some(initial_highlight));
            } else {
                self.highlighted = None;
            }
        }
    }

    /// Move selection up in dropdown
    pub fn move_up(&mut self) {
        if self.open && !self.options.is_empty() {
            let current = self.highlighted.unwrap_or(0);
            let new_highlighted = if current > 0 {
                current - 1
            } else {
                self.options.len() - 1
            };
            self.highlighted = Some(new_highlighted);
            self.list_state.select(Some(new_highlighted));
        }
    }

    /// Move selection down in dropdown
    pub fn move_down(&mut self) {
        if self.open && !self.options.is_empty() {
            let current = self.highlighted.unwrap_or(0);
            let new_highlighted = if current + 1 < self.options.len() {
                current + 1
            } else {
                0
            };
            self.highlighted = Some(new_highlighted);
            self.list_state.select(Some(new_highlighted));
        }
    }

    /// Select the currently highlighted option
    pub fn select_current(&mut self) {
        if self.open {
            if let Some(highlighted) = self.highlighted {
                if highlighted < self.options.len() && self.options[highlighted].enabled {
                    self.selected = Some(highlighted);
                    self.open = false;
                    self.highlighted = None;
                }
            }
        }
    }

    /// Get the selected value
    pub fn selected_value(&self) -> Option<&T> {
        self.selected
            .and_then(|idx| self.options.get(idx))
            .map(|opt| &opt.value)
    }

    /// Get the selected option text
    pub fn selected_text(&self) -> Option<&str> {
        self.selected
            .and_then(|idx| self.options.get(idx))
            .map(|opt| opt.text.as_str())
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Select option by value
    pub fn select_by_value(&mut self, value: &T) -> bool
    where
        T: PartialEq,
    {
        if let Some((index, _)) = self
            .options
            .iter()
            .enumerate()
            .find(|(_, opt)| &opt.value == value)
        {
            self.selected = Some(index);
            true
        } else {
            false
        }
    }

    /// Render the dropdown
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Label
                Constraint::Length(3), // Selected value box
                Constraint::Min(0),    // Dropdown list (when open)
            ])
            .split(area);

        // Render label
        let label_style = if self.required {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let label_text = if self.required {
            format!("{} *", self.label)
        } else {
            self.label.clone()
        };

        frame.render_widget(Paragraph::new(label_text).style(label_style), chunks[0]);

        // Render selected value box
        let box_style = if self.focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let selected_text = if self.options.is_empty() {
            "Loading options...".to_string()
        } else {
            self.selected_text()
                .unwrap_or("Select an option...")
                .to_string()
        };

        let arrow = if self.open { "▲" } else { "▼" };
        let display_text = format!("{} {}", selected_text, arrow);

        let block = Block::default().borders(Borders::ALL).style(box_style);

        frame.render_widget(
            Paragraph::new(display_text)
                .block(block)
                .style(Style::default().fg(Color::White)),
            chunks[1],
        );

        // Render dropdown list if open
        if self.open && !self.options.is_empty() {
            let list_height = (self.options.len() as u16 + 2).min(8); // Max 8 items visible
            let list_area = Rect {
                x: chunks[1].x,
                y: chunks[1].y + chunks[1].height,
                width: chunks[1].width,
                height: list_height,
            };

            // Clear background
            frame.render_widget(Clear, list_area);

            let items: Vec<ListItem> = self
                .options
                .iter()
                .enumerate()
                .map(|(idx, opt)| {
                    let style = if !opt.enabled {
                        Style::default().fg(Color::DarkGray)
                    } else if Some(idx) == self.highlighted {
                        Style::default().fg(Color::Black).bg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let text = if Some(idx) == self.selected {
                        format!("✓ {}", opt.text)
                    } else {
                        format!("  {}", opt.text)
                    };

                    ListItem::new(text).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Select Option")
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));

            frame.render_stateful_widget(list, list_area, &mut self.list_state);
        }
    }
}

/// Checkbox component for feature toggles and confirmations
#[derive(Debug, Clone)]
pub struct Checkbox {
    /// Label for the checkbox
    label: String,
    /// Whether the checkbox is checked
    checked: bool,
    /// Whether the checkbox is focused
    focused: bool,
    /// Whether the checkbox is enabled
    enabled: bool,
}

impl Checkbox {
    /// Create a new checkbox with a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
            focused: false,
            enabled: true,
        }
    }

    /// Set the checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set the enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Toggle the checkbox state
    pub fn toggle(&mut self) {
        if self.enabled {
            self.checked = !self.checked;
        }
    }

    /// Get the checked state
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Render the checkbox
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let checkbox_symbol = if self.checked { "☑" } else { "☐" };

        let style = if !self.enabled {
            Style::default().fg(Color::DarkGray)
        } else if self.focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let text = format!("{} {}", checkbox_symbol, self.label);

        frame.render_widget(Paragraph::new(text).style(style), area);
    }
}

/// Toggle switch component for feature toggles
#[derive(Debug, Clone)]
pub struct Toggle {
    /// Label for the toggle
    label: String,
    /// Whether the toggle is on
    enabled: bool,
    /// Whether the toggle is focused
    focused: bool,
    /// Whether the toggle can be interacted with
    interactive: bool,
}

impl Toggle {
    /// Create a new toggle with a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: false,
            focused: false,
            interactive: true,
        }
    }

    /// Set the enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the interactive state
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Toggle the switch state
    pub fn toggle(&mut self) {
        if self.interactive {
            self.enabled = !self.enabled;
        }
    }

    /// Get the enabled state
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Render the toggle
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let toggle_display = if self.enabled { "[ON]" } else { "[OFF]" };

        let toggle_color = if self.enabled {
            Color::Green
        } else {
            Color::Red
        };

        let style = if !self.interactive {
            Style::default().fg(Color::DarkGray)
        } else if self.focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let toggle_style = if self.focused {
            Style::default()
                .fg(toggle_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(toggle_color)
        };

        let spans = vec![
            Span::styled(&self.label, style),
            Span::raw(" "),
            Span::styled(toggle_display, toggle_style),
        ];

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_validation() {
        let mut input = TextInput::new("Test").with_type(InputType::Amount);

        // Test valid amount
        input.input = Input::default().with_value("123.45".to_string());
        assert!(input.validate());

        // Test invalid amount
        input.input = Input::default().with_value("not_a_number".to_string());
        assert!(!input.validate());
        assert!(input.error().unwrap().contains("Invalid amount format"));
    }

    #[test]
    fn test_dropdown_selection() {
        let mut dropdown = Dropdown::new("Test")
            .add_option(DropdownOption::new("Option 1", 1))
            .add_option(DropdownOption::new("Option 2", 2));

        dropdown.set_focused(true);
        dropdown.toggle(); // Open dropdown
        dropdown.move_down(); // Select second option
        dropdown.select_current();

        assert_eq!(dropdown.selected_value(), Some(&2));
        assert_eq!(dropdown.selected_text(), Some("Option 2"));
    }

    #[test]
    fn test_checkbox_toggle() {
        let mut checkbox = Checkbox::new("Test");
        assert!(!checkbox.is_checked());

        checkbox.toggle();
        assert!(checkbox.is_checked());

        checkbox.toggle();
        assert!(!checkbox.is_checked());
    }
}
