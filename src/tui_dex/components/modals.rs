//! Modal/Popup Components
//!
//! This module provides modal and popup dialog components for confirmations,
//! details display, and user input overlays.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Padding, Paragraph, Wrap},
};

/// Modal types for different use cases
#[derive(Debug, Clone)]
pub enum ModalType {
    Confirmation {
        title: String,
        message: String,
        confirm_text: String,
        cancel_text: String,
    },
    Information {
        title: String,
        content: Vec<String>,
    },
    Error {
        title: String,
        error_message: String,
        details: Option<Vec<String>>,
        error_type: ErrorType,
        retry_action: Option<String>,
    },
    TransactionDetails {
        tx_hash: String,
        status: String,
        details: Vec<(String, String)>,
    },
    Help {
        title: String,
        sections: Vec<HelpSection>,
    },
    Loading {
        title: String,
        message: String,
        progress: Option<f64>, // 0.0 to 100.0, None for indeterminate
        can_cancel: bool,
    },
    ValidationError {
        title: String,
        field_name: String,
        error_message: String,
        suggestions: Vec<String>,
    },
    WalletSave {
        title: String,
        message: String,
        mnemonic: String,
        address: String,
        wallet_name: String,
        password: String,
        confirm_password: String,
        current_field: WalletSaveField,
        show_password: bool,
    },
}

/// Error types for better error categorization and handling
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorType {
    Network,
    Validation,
    Transaction,
    Configuration,
    Wallet,
    Contract,
    Authentication,
    InsufficientFunds,
    Timeout,
    Unknown,
}

impl ErrorType {
    /// Get a user-friendly description of the error type
    pub fn description(&self) -> &'static str {
        match self {
            ErrorType::Network => "Network connectivity issue - check your internet connection",
            ErrorType::Validation => "Input validation failed - please check your input values",
            ErrorType::Transaction => "Blockchain transaction failed - check logs for details",
            ErrorType::Configuration => "Configuration error - check your settings",
            ErrorType::Wallet => "Wallet operation failed - check wallet connection",
            ErrorType::Contract => "Smart contract interaction failed",
            ErrorType::Authentication => "Authentication failed - check credentials or permissions",
            ErrorType::InsufficientFunds => "Insufficient funds - check account balance",
            ErrorType::Timeout => "Operation timed out - network may be slow or unavailable",
            ErrorType::Unknown => "An unexpected error occurred",
        }
    }

    /// Get suggested actions for this error type
    pub fn suggested_actions(&self) -> Vec<&'static str> {
        match self {
            ErrorType::Network => vec![
                "Check internet connection",
                "Verify RPC endpoint",
                "Try different network",
                "Wait and retry",
            ],
            ErrorType::Validation => vec![
                "Review input values",
                "Check format requirements",
                "Ensure required fields are filled",
            ],
            ErrorType::Transaction => vec![
                "Check wallet balance",
                "Verify gas settings",
                "Review transaction parameters",
                "Check network status",
            ],
            ErrorType::Configuration => vec![
                "Review network settings",
                "Check contract addresses",
                "Verify configuration file",
            ],
            ErrorType::Wallet => vec![
                "Check wallet connection",
                "Verify wallet balance",
                "Confirm wallet is unlocked",
            ],
            ErrorType::Contract => vec![
                "Check contract status",
                "Verify contract address",
                "Review function parameters",
            ],
            ErrorType::Authentication => vec![
                "Check credentials",
                "Verify permissions",
                "Ensure proper authorization",
                "Contact administrator if needed",
            ],
            ErrorType::InsufficientFunds => vec![
                "Check account balance",
                "Add funds to account",
                "Reduce transaction amount",
                "Check minimum balance requirements",
            ],
            ErrorType::Timeout => vec![
                "Wait and retry operation",
                "Check network connection",
                "Try with shorter timeout",
                "Use different RPC endpoint",
            ],
            ErrorType::Unknown => vec![
                "Review logs for details",
                "Try operation again",
                "Contact support if persistent",
            ],
        }
    }
}

/// Fields in wallet save modal
#[derive(Debug, Clone, PartialEq)]
pub enum WalletSaveField {
    WalletName,
    Password,
    ConfirmPassword,
    SaveButton,
    CancelButton,
}

/// Help section for help modal
#[derive(Debug, Clone)]
pub struct HelpSection {
    pub title: String,
    pub items: Vec<(String, String)>, // (key, description) pairs
}

/// Modal state
#[derive(Debug, Clone)]
pub struct ModalState {
    pub modal_type: ModalType,
    pub is_visible: bool,
    pub selected_option: usize, // For confirmation dialogs and options
    pub scroll_offset: usize,   // For scrolling in help and error details
}

impl ModalState {
    /// Create a new confirmation modal
    pub fn confirmation(
        title: String,
        message: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    ) -> Self {
        Self {
            modal_type: ModalType::Confirmation {
                title,
                message,
                confirm_text: confirm_text.unwrap_or_else(|| "Yes".to_string()),
                cancel_text: cancel_text.unwrap_or_else(|| "No".to_string()),
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new information modal
    pub fn information(title: String, content: Vec<String>) -> Self {
        Self {
            modal_type: ModalType::Information { title, content },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new error modal with comprehensive error handling
    pub fn error(
        title: String,
        error_message: String,
        error_type: ErrorType,
        details: Option<Vec<String>>,
        retry_action: Option<String>,
    ) -> Self {
        Self {
            modal_type: ModalType::Error {
                title,
                error_message,
                details,
                error_type,
                retry_action,
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a validation error modal with suggestions
    pub fn validation_error(
        title: String,
        field_name: String,
        error_message: String,
        suggestions: Vec<String>,
    ) -> Self {
        Self {
            modal_type: ModalType::ValidationError {
                title,
                field_name,
                error_message,
                suggestions,
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new transaction details modal
    pub fn transaction_details(
        tx_hash: String,
        status: String,
        details: Vec<(String, String)>,
    ) -> Self {
        Self {
            modal_type: ModalType::TransactionDetails {
                tx_hash,
                status,
                details,
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new help modal
    pub fn help(title: String, sections: Vec<HelpSection>) -> Self {
        Self {
            modal_type: ModalType::Help { title, sections },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new loading modal
    pub fn loading(
        title: String,
        message: String,
        progress: Option<f64>,
        can_cancel: bool,
    ) -> Self {
        Self {
            modal_type: ModalType::Loading {
                title,
                message,
                progress,
                can_cancel,
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Create a new wallet save modal
    pub fn wallet_save(title: String, message: String, mnemonic: String, address: String) -> Self {
        Self {
            modal_type: ModalType::WalletSave {
                title,
                message,
                mnemonic,
                address,
                wallet_name: String::new(),
                password: String::new(),
                confirm_password: String::new(),
                current_field: WalletSaveField::WalletName,
                show_password: false,
            },
            is_visible: true,
            selected_option: 0,
            scroll_offset: 0,
        }
    }

    /// Hide the modal
    pub fn hide(&mut self) {
        self.is_visible = false;
    }

    /// Show the modal
    pub fn show(&mut self) {
        self.is_visible = true;
    }

    /// Move selection up (for confirmation dialogs and error options)
    pub fn select_previous(&mut self) {
        if self.selected_option > 0 {
            self.selected_option -= 1;
        }
    }

    /// Move selection down (for confirmation dialogs and error options)
    pub fn select_next(&mut self) {
        let max_options = match &self.modal_type {
            ModalType::Confirmation { .. } => 1,
            ModalType::Error { retry_action, .. } => {
                if retry_action.is_some() {
                    2
                } else {
                    0
                }
            }
            ModalType::Loading { can_cancel, .. } => {
                if *can_cancel {
                    0
                } else {
                    usize::MAX
                } // No selection for non-cancelable loading
            }
            _ => 0,
        };

        if max_options > 0 && self.selected_option < max_options {
            self.selected_option += 1;
        }
    }

    /// Scroll content up (for help and detailed error modals)
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll content down (for help and detailed error modals)
    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    /// Check if the modal is currently visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Get the currently selected option (true for confirm, false for cancel)
    pub fn is_confirmed(&self) -> bool {
        self.selected_option == 0
    }

    /// Check if retry was selected for error modals
    pub fn is_retry_selected(&self) -> bool {
        match &self.modal_type {
            ModalType::Error { retry_action, .. } => {
                retry_action.is_some() && self.selected_option == 0
            }
            _ => false,
        }
    }

    /// Update loading progress
    pub fn update_progress(&mut self, progress: Option<f64>) {
        if let ModalType::Loading {
            progress: ref mut p,
            ..
        } = self.modal_type
        {
            *p = progress;
        }
    }

    /// Update loading message
    pub fn update_loading_message(&mut self, message: String) {
        if let ModalType::Loading {
            message: ref mut m, ..
        } = self.modal_type
        {
            *m = message;
        }
    }
}

/// Create a comprehensive help modal with all keyboard shortcuts
pub fn create_comprehensive_help() -> ModalState {
    let sections = vec![
        HelpSection {
            title: "Navigation".to_string(),
            items: vec![
                ("Tab".to_string(), "Next screen".to_string()),
                ("Shift+Tab".to_string(), "Previous screen".to_string()),
                ("↑/↓".to_string(), "Navigate lists".to_string()),
                ("←/→".to_string(), "Navigate horizontally".to_string()),
                ("Enter".to_string(), "Confirm/Execute action".to_string()),
                ("Esc".to_string(), "Cancel/Go back".to_string()),
            ],
        },
        HelpSection {
            title: "Global Actions".to_string(),
            items: vec![
                ("q".to_string(), "Quit application".to_string()),
                ("h, F1".to_string(), "Show this help".to_string()),
                ("r, F5".to_string(), "Refresh current screen".to_string()),
                ("Ctrl+C".to_string(), "Force quit".to_string()),
            ],
        },
        HelpSection {
            title: "Swap Screen".to_string(),
            items: vec![
                ("s".to_string(), "Execute swap".to_string()),
                ("p".to_string(), "Preview swap".to_string()),
                ("c".to_string(), "Clear form".to_string()),
            ],
        },
        HelpSection {
            title: "Liquidity Screen".to_string(),
            items: vec![
                ("l".to_string(), "Provide liquidity".to_string()),
                ("w".to_string(), "Withdraw liquidity".to_string()),
                ("v".to_string(), "View positions".to_string()),
            ],
        },
        HelpSection {
            title: "Rewards Screen".to_string(),
            items: vec![
                ("1".to_string(), "Claim all rewards".to_string()),
                ("2".to_string(), "Claim until epoch".to_string()),
                ("0-9".to_string(), "Enter epoch number".to_string()),
            ],
        },
        HelpSection {
            title: "Admin Screen".to_string(),
            items: vec![
                ("m".to_string(), "Pool management".to_string()),
                ("c".to_string(), "Create pool".to_string()),
                ("f".to_string(), "Feature controls".to_string()),
            ],
        },
    ];

    ModalState::help("MANTRA DEX - Keyboard Shortcuts".to_string(), sections)
}

/// Render modal overlay
pub fn render_modal(f: &mut Frame, modal_state: &ModalState, area: Rect) {
    if !modal_state.is_visible {
        return;
    }

    // Calculate modal size and position (centered)
    let modal_area = match &modal_state.modal_type {
        ModalType::Help { .. } => centered_rect(80, 70, area),
        ModalType::Error { details, .. } => {
            if details.is_some() {
                centered_rect(70, 60, area)
            } else {
                centered_rect(60, 40, area)
            }
        }
        ModalType::Loading { .. } => centered_rect(50, 30, area),
        ModalType::TransactionDetails { .. } => centered_rect(80, 60, area),
        _ => centered_rect(60, 40, area),
    };

    // Clear the area behind the modal
    f.render_widget(Clear, modal_area);

    match &modal_state.modal_type {
        ModalType::Confirmation {
            title,
            message,
            confirm_text,
            cancel_text,
        } => render_confirmation_modal(
            f,
            title,
            message,
            confirm_text,
            cancel_text,
            modal_state.selected_option,
            modal_area,
        ),
        ModalType::Information { title, content } => {
            render_information_modal(f, title, content, modal_area)
        }
        ModalType::Error {
            title,
            error_message,
            details,
            error_type,
            retry_action,
        } => render_comprehensive_error_modal(
            f,
            title,
            error_message,
            details,
            error_type,
            retry_action,
            modal_state.selected_option,
            modal_state.scroll_offset,
            modal_area,
        ),
        ModalType::ValidationError {
            title,
            field_name,
            error_message,
            suggestions,
        } => render_validation_error_modal(
            f,
            title,
            field_name,
            error_message,
            suggestions,
            modal_area,
        ),
        ModalType::TransactionDetails {
            tx_hash,
            status,
            details,
        } => render_transaction_modal(f, tx_hash, status, details, modal_area),
        ModalType::Help { title, sections } => render_comprehensive_help_modal(
            f,
            title,
            sections,
            modal_state.scroll_offset,
            modal_area,
        ),
        ModalType::Loading {
            title,
            message,
            progress,
            can_cancel,
        } => render_loading_modal(f, title, message, progress, *can_cancel, modal_area),

        ModalType::WalletSave {
            title,
            message,
            wallet_name,
            password,
            confirm_password,
            current_field,
            show_password,
            ..
        } => render_wallet_save_modal(
            f,
            title,
            message,
            wallet_name,
            password,
            confirm_password,
            current_field,
            *show_password,
            modal_area,
        ),
    }
}

/// Render confirmation modal
fn render_confirmation_modal(
    f: &mut Frame,
    title: &str,
    message: &str,
    confirm_text: &str,
    cancel_text: &str,
    selected_option: usize,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Message area
            Constraint::Length(3), // Button area
        ])
        .split(area);

    // Message area
    let message_paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title)
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(message_paragraph, chunks[0]);

    // Button area
    let button_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Confirm button
    let confirm_style = if selected_option == 0 {
        Style::default().bg(Color::Green).fg(Color::Black)
    } else {
        Style::default().fg(Color::Green)
    };

    let confirm_button = Paragraph::new(confirm_text)
        .style(confirm_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(confirm_button, button_chunks[0]);

    // Cancel button
    let cancel_style = if selected_option == 1 {
        Style::default().bg(Color::Red).fg(Color::Black)
    } else {
        Style::default().fg(Color::Red)
    };

    let cancel_button = Paragraph::new(cancel_text)
        .style(cancel_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(cancel_button, button_chunks[1]);
}

/// Render information modal
fn render_information_modal(f: &mut Frame, title: &str, content: &[String], area: Rect) {
    let items: Vec<ListItem> = content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Render comprehensive error modal with retry options and detailed guidance
fn render_comprehensive_error_modal(
    f: &mut Frame,
    title: &str,
    error_message: &str,
    details: &Option<Vec<String>>,
    error_type: &ErrorType,
    retry_action: &Option<String>,
    selected_option: usize,
    scroll_offset: usize,
    area: Rect,
) {
    let has_actions = retry_action.is_some();
    let constraints = if has_actions {
        vec![
            Constraint::Length(4), // Error message
            Constraint::Length(3), // Error type description
            Constraint::Min(0),    // Details and suggestions
            Constraint::Length(3), // Action buttons
        ]
    } else {
        vec![
            Constraint::Length(4), // Error message
            Constraint::Length(3), // Error type description
            Constraint::Min(0),    // Details and suggestions
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Error message
    let error_paragraph = Paragraph::new(error_message)
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(title)
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(error_paragraph, chunks[0]);

    // Error type description
    let type_paragraph = Paragraph::new(error_type.description())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Error Type")
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(type_paragraph, chunks[1]);

    // Details and suggestions
    let mut content = Vec::new();

    if let Some(detail_lines) = details {
        content.push("Details:".to_string());
        content.extend(detail_lines.clone());
        content.push("".to_string());
    }

    content.push("Suggested Actions:".to_string());
    content.extend(
        error_type
            .suggested_actions()
            .iter()
            .map(|s| format!("• {}", s)),
    );

    let visible_content: Vec<_> = content
        .iter()
        .skip(scroll_offset)
        .take((chunks[2].height.saturating_sub(2)) as usize)
        .collect();

    let items: Vec<ListItem> = visible_content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title("Details & Suggestions"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, chunks[2]);

    // Action buttons (if available)
    if has_actions {
        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[3]);

        // Retry button
        let retry_style = if selected_option == 0 {
            Style::default().bg(Color::Green).fg(Color::Black)
        } else {
            Style::default().fg(Color::Green)
        };

        let default_retry = "Retry".to_string();
        let retry_text = retry_action.as_ref().unwrap_or(&default_retry);
        let retry_button = Paragraph::new(retry_text.as_str())
            .style(retry_style)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(retry_button, button_chunks[0]);

        // Cancel button
        let cancel_style = if selected_option == 1 {
            Style::default().bg(Color::Red).fg(Color::Black)
        } else {
            Style::default().fg(Color::Red)
        };

        let cancel_button = Paragraph::new("Cancel")
            .style(cancel_style)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(cancel_button, button_chunks[1]);
    }
}

/// Render validation error modal with field-specific guidance
fn render_validation_error_modal(
    f: &mut Frame,
    title: &str,
    field_name: &str,
    error_message: &str,
    suggestions: &[String],
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Error message
            Constraint::Min(0),    // Suggestions
        ])
        .split(area);

    // Error message with field context
    let full_message = format!("Field: {}\n\nError: {}", field_name, error_message);
    let error_paragraph = Paragraph::new(full_message)
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(title)
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(error_paragraph, chunks[0]);

    // Suggestions
    let mut content = vec!["Suggestions to fix this error:".to_string(), "".to_string()];
    content.extend(suggestions.iter().map(|s| format!("• {}", s)));

    let items: Vec<ListItem> = content
        .iter()
        .map(|line| ListItem::new(line.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("How to Fix"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, chunks[1]);
}

/// Render loading modal with progress indication
fn render_loading_modal(
    f: &mut Frame,
    title: &str,
    message: &str,
    progress: &Option<f64>,
    can_cancel: bool,
    area: Rect,
) {
    let constraints = if can_cancel {
        vec![
            Constraint::Length(3), // Message
            Constraint::Length(3), // Progress bar
            Constraint::Length(3), // Cancel button
        ]
    } else {
        vec![
            Constraint::Length(3), // Message
            Constraint::Length(3), // Progress bar
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Message
    let message_paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(title),
        )
        .alignment(Alignment::Center);

    f.render_widget(message_paragraph, chunks[0]);

    // Progress bar
    let progress_percent = progress.unwrap_or(50.0) as u16;
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Progress"),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(progress_percent)
        .label(if progress.is_some() {
            format!("{}%", progress_percent)
        } else {
            "Processing...".to_string()
        });

    f.render_widget(gauge, chunks[1]);

    // Cancel button (if available)
    if can_cancel {
        let cancel_button = Paragraph::new("Press Esc to Cancel")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(cancel_button, chunks[2]);
    }
}

/// Render transaction details modal
fn render_transaction_modal(
    f: &mut Frame,
    tx_hash: &str,
    status: &str,
    details: &[(String, String)],
    area: Rect,
) {
    let status_color = match status.to_lowercase().as_str() {
        "success" => Color::Green,
        "failed" => Color::Red,
        "pending" => Color::Yellow,
        _ => Color::Gray,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with hash and status
            Constraint::Min(0),    // Details
        ])
        .split(area);

    // Header
    let header_text = format!("Hash: {}\nStatus: {}", tx_hash, status);
    let header_paragraph = Paragraph::new(header_text)
        .style(Style::default().fg(status_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(status_color))
                .title("Transaction"),
        )
        .alignment(Alignment::Center);

    f.render_widget(header_paragraph, chunks[0]);

    // Details with text wrapping for long URLs
    let items: Vec<ListItem> = details
        .iter()
        .map(|(key, value)| {
            // For long values (like explorer URLs), wrap them
            if value.len() > 60 {
                let wrapped_value = format!("{}\n  {}", key, value);
                ListItem::new(wrapped_value)
            } else {
                ListItem::new(format!("{}: {}", key, value))
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Details"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, chunks[1]);
}

/// Render comprehensive help modal with scrolling support
fn render_comprehensive_help_modal(
    f: &mut Frame,
    title: &str,
    sections: &[HelpSection],
    scroll_offset: usize,
    area: Rect,
) {
    let mut content = Vec::new();

    for section in sections {
        content.push(format!("═══ {} ═══", section.title));
        content.push("".to_string());

        for (key, description) in &section.items {
            content.push(format!("  {:<12} {}", key, description));
        }
        content.push("".to_string());
    }

    // Add navigation instructions
    content.push("═══ Navigation ═══".to_string());
    content.push("".to_string());
    content.push("  ↑/↓          Scroll help content".to_string());
    content.push("  Esc           Close help".to_string());

    // Apply scrolling
    let visible_content: Vec<_> = content
        .iter()
        .skip(scroll_offset)
        .take((area.height.saturating_sub(2)) as usize)
        .collect();

    let items: Vec<ListItem> = visible_content
        .iter()
        .map(|line| {
            if line.starts_with("═══") {
                ListItem::new(line.as_str()).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else if line.starts_with("  ") {
                ListItem::new(line.as_str()).style(Style::default().fg(Color::White))
            } else {
                ListItem::new(line.as_str()).style(Style::default().fg(Color::Yellow))
            }
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(title),
    );

    f.render_widget(list, area);
}

/// Helper function to create a centered rectangle
/// Render wallet save modal with form fields
fn render_wallet_save_modal(
    f: &mut Frame,
    title: &str,
    message: &str,
    wallet_name: &str,
    password: &str,
    confirm_password: &str,
    current_field: &WalletSaveField,
    show_password: bool,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Message
            Constraint::Length(3), // Wallet name input
            Constraint::Length(3), // Password input
            Constraint::Length(3), // Confirm password input
            Constraint::Length(3), // Buttons
        ])
        .split(area);

    // Message
    let message_paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title)
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(message_paragraph, chunks[0]);

    // Wallet name input
    let name_style = if *current_field == WalletSaveField::WalletName {
        Style::default().bg(Color::Blue).fg(Color::White)
    } else {
        Style::default().fg(Color::White)
    };

    let name_input = Paragraph::new(wallet_name).style(name_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Wallet Name")
            .padding(Padding::uniform(1)),
    );

    f.render_widget(name_input, chunks[1]);

    // Password input
    let password_style = if *current_field == WalletSaveField::Password {
        Style::default().bg(Color::Blue).fg(Color::White)
    } else {
        Style::default().fg(Color::White)
    };

    let password_display = if show_password {
        password.to_string()
    } else {
        "*".repeat(password.len())
    };

    let password_input = Paragraph::new(password_display)
        .style(password_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Password (12+ chars, mixed case, numbers, symbols)")
                .padding(Padding::uniform(1)),
        );

    f.render_widget(password_input, chunks[2]);

    // Confirm password input
    let confirm_style = if *current_field == WalletSaveField::ConfirmPassword {
        Style::default().bg(Color::Blue).fg(Color::White)
    } else {
        Style::default().fg(Color::White)
    };

    let confirm_display = if show_password {
        confirm_password.to_string()
    } else {
        "*".repeat(confirm_password.len())
    };

    let confirm_input = Paragraph::new(confirm_display).style(confirm_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Confirm Password")
            .padding(Padding::uniform(1)),
    );

    f.render_widget(confirm_input, chunks[3]);

    // Buttons
    let button_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[4]);

    // Save button
    let save_style = if *current_field == WalletSaveField::SaveButton {
        Style::default().bg(Color::Green).fg(Color::Black)
    } else {
        Style::default().fg(Color::Green)
    };

    let save_button = Paragraph::new("Save Wallet")
        .style(save_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(save_button, button_chunks[0]);

    // Cancel button
    let cancel_style = if *current_field == WalletSaveField::CancelButton {
        Style::default().bg(Color::Red).fg(Color::Black)
    } else {
        Style::default().fg(Color::Red)
    };

    let cancel_button = Paragraph::new("Skip")
        .style(cancel_style)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(cancel_button, button_chunks[1]);
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_state_creation() {
        let modal = ModalState::confirmation("Test".to_string(), "Message".to_string(), None, None);
        assert!(modal.is_visible());
        assert!(modal.is_confirmed());
    }

    #[test]
    fn test_modal_navigation() {
        let mut modal =
            ModalState::confirmation("Test".to_string(), "Message".to_string(), None, None);
        assert_eq!(modal.selected_option, 0);

        modal.select_next();
        assert_eq!(modal.selected_option, 1);

        modal.select_previous();
        assert_eq!(modal.selected_option, 0);
    }

    #[test]
    fn test_error_type_suggestions() {
        let network_error = ErrorType::Network;
        let actions = network_error.suggested_actions();
        assert!(!actions.is_empty());
        assert!(actions.contains(&"Check internet connection"));
    }

    #[test]
    fn test_comprehensive_help_creation() {
        let help_modal = create_comprehensive_help();
        assert!(help_modal.is_visible());
        match help_modal.modal_type {
            ModalType::Help { sections, .. } => {
                assert!(!sections.is_empty());
                assert!(sections.iter().any(|s| s.title == "Navigation"));
            }
            _ => panic!("Expected Help modal type"),
        }
    }
}
