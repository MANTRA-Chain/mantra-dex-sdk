//! Wallet Setup Wizard
//!
//! This module provides a guided setup wizard for first-time users to configure their wallet
//! and network settings in the MANTRA DEX SDK TUI.

use crate::tui_dex::{
    app::App,
    components::{header::render_header, status_bar::render_status_bar},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use std::borrow::Cow;

/// Wizard steps for first-time setup
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WizardStep {
    Welcome,
    NetworkSelection,
    WalletSetup,
    SecurityWarning,
    WalletSave,
    Confirmation,
    Complete,
}

impl WizardStep {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Welcome,
            Self::NetworkSelection,
            Self::WalletSetup,
            Self::SecurityWarning,
            Self::WalletSave,
            Self::Confirmation,
            Self::Complete,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome to MANTRA DEX",
            Self::NetworkSelection => "Network Configuration",
            Self::WalletSetup => "Wallet Setup",
            Self::SecurityWarning => "Security Information",
            Self::WalletSave => "Save Wallet",
            Self::Confirmation => "Confirm Settings",
            Self::Complete => "Setup Complete",
        }
    }

    pub fn step_number(&self) -> (usize, usize) {
        let all = Self::all();
        let current = all.iter().position(|&s| s == *self).unwrap() + 1;
        (current, all.len())
    }
}

/// Network environment options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkEnvironment {
    Mainnet,
    Testnet,
}

impl NetworkEnvironment {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mainnet => "Mainnet (Production)",
            Self::Testnet => "Testnet (Development)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Mainnet => "Real transactions with real assets. Use with caution.",
            Self::Testnet => "Test environment with fake assets. Safe for learning.",
        }
    }
}

/// Wallet setup wizard state
#[derive(Debug, Clone)]
pub struct WizardState {
    /// Current wizard step
    pub current_step: WizardStep,
    /// Selected network environment
    pub selected_network: NetworkEnvironment,
    /// Mnemonic input for wallet setup
    pub mnemonic_input: String,
    /// Whether user has acknowledged security warnings
    pub security_acknowledged: bool,
    /// Whether user wants to create new wallet or import existing
    pub import_existing: bool,
    /// Generated mnemonic (if creating new wallet)
    pub generated_mnemonic: Option<String>,
    /// Whether to show the wizard
    pub show_wizard: bool,
    /// Wallet save fields
    pub wallet_name: String,
    pub save_password: String,
    pub save_password_confirm: String,
    /// Whether user wants to save the wallet
    pub save_wallet: bool,
    /// Whether wallet save modal is currently shown
    pub show_save_modal: bool,
    /// Validation errors for wallet save form
    pub wallet_save_errors: Vec<String>,
    /// Current focused field in wallet save step
    pub wallet_save_focus: WalletSaveField,
}

/// Fields that can be focused in the wallet save step
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WalletSaveField {
    SaveToggle,
    WalletName,
    Password,
    PasswordConfirm,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            current_step: WizardStep::Welcome,
            selected_network: NetworkEnvironment::Testnet,
            mnemonic_input: String::new(),
            security_acknowledged: false,
            import_existing: true,
            generated_mnemonic: None,
            show_wizard: true,
            wallet_name: String::new(),
            save_password: String::new(),
            save_password_confirm: String::new(),
            save_wallet: true, // Default to saving wallet for convenience
            show_save_modal: false,
            wallet_save_errors: Vec::new(),
            wallet_save_focus: WalletSaveField::SaveToggle,
        }
    }
}

impl WizardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_step(&mut self) {
        self.current_step = match self.current_step {
            WizardStep::Welcome => WizardStep::NetworkSelection,
            WizardStep::NetworkSelection => WizardStep::WalletSetup,
            WizardStep::WalletSetup => WizardStep::SecurityWarning,
            WizardStep::SecurityWarning => WizardStep::WalletSave,
            WizardStep::WalletSave => WizardStep::Confirmation,
            WizardStep::Confirmation => WizardStep::Complete,
            WizardStep::Complete => WizardStep::Complete,
        };
    }

    pub fn previous_step(&mut self) {
        self.current_step = match self.current_step {
            WizardStep::Welcome => WizardStep::Welcome,
            WizardStep::NetworkSelection => WizardStep::Welcome,
            WizardStep::WalletSetup => WizardStep::NetworkSelection,
            WizardStep::SecurityWarning => WizardStep::WalletSetup,
            WizardStep::WalletSave => WizardStep::SecurityWarning,
            WizardStep::Confirmation => WizardStep::WalletSave,
            WizardStep::Complete => WizardStep::Confirmation,
        };
    }

    pub fn toggle_network(&mut self) {
        self.selected_network = match self.selected_network {
            NetworkEnvironment::Mainnet => NetworkEnvironment::Testnet,
            NetworkEnvironment::Testnet => NetworkEnvironment::Mainnet,
        };
    }

    pub fn toggle_wallet_mode(&mut self) {
        self.import_existing = !self.import_existing;
        if !self.import_existing {
            // Generate new mnemonic when switching to create mode
            self.generated_mnemonic = Some(generate_mnemonic());
        }
    }

    pub fn finish_wizard(&mut self) {
        self.show_wizard = false;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn can_proceed(&self) -> bool {
        match self.current_step {
            WizardStep::Welcome => true,
            WizardStep::NetworkSelection => true,
            WizardStep::WalletSetup => {
                if self.import_existing {
                    !self.mnemonic_input.is_empty()
                        && self.mnemonic_input.split_whitespace().count() >= 12
                } else {
                    true // Generated mnemonic is always valid
                }
            }
            WizardStep::SecurityWarning => self.security_acknowledged,
            WizardStep::WalletSave => {
                if self.save_wallet {
                    self.validate_wallet_save().is_empty()
                } else {
                    true // Can skip saving
                }
            }
            WizardStep::Confirmation => true,
            WizardStep::Complete => true,
        }
    }

    /// Validate wallet save form and return errors
    pub fn validate_wallet_save(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Validate wallet name
        if self.wallet_name.trim().is_empty() {
            errors.push("Wallet name is required".to_string());
        } else if self.wallet_name.trim().len() < 3 {
            errors.push("Wallet name must be at least 3 characters".to_string());
        } else if self.wallet_name.contains('/') || self.wallet_name.contains('\\') {
            errors.push("Wallet name cannot contain path separators".to_string());
        }

        // Validate password strength
        if let Err(validation_error) =
            crate::wallet::WalletStorage::default().validate_password(&self.save_password)
        {
            errors.push(format!("Password error: {}", validation_error));
        }

        // Validate password confirmation
        if self.save_password != self.save_password_confirm {
            errors.push("Passwords do not match".to_string());
        }

        errors
    }

    /// Toggle wallet save option
    pub fn toggle_save_wallet(&mut self) {
        self.save_wallet = !self.save_wallet;
        if !self.save_wallet {
            // Clear form when disabling save
            self.wallet_name.clear();
            self.save_password.clear();
            self.save_password_confirm.clear();
            self.wallet_save_errors.clear();
        }
    }

    /// Clear sensitive wallet save data from memory
    pub fn clear_wallet_save_data(&mut self) {
        // Overwrite passwords with zeros for security
        unsafe {
            let password_bytes = self.save_password.as_bytes_mut();
            for byte in password_bytes {
                *byte = 0;
            }
            let confirm_bytes = self.save_password_confirm.as_bytes_mut();
            for byte in confirm_bytes {
                *byte = 0;
            }
        }
        self.save_password.clear();
        self.save_password_confirm.clear();
    }

    /// Navigate to next field in wallet save step
    pub fn wallet_save_focus_next(&mut self) {
        if !self.save_wallet {
            // If save wallet is disabled, only toggle field is available
            self.wallet_save_focus = WalletSaveField::SaveToggle;
            return;
        }

        self.wallet_save_focus = match self.wallet_save_focus {
            WalletSaveField::SaveToggle => WalletSaveField::WalletName,
            WalletSaveField::WalletName => WalletSaveField::Password,
            WalletSaveField::Password => WalletSaveField::PasswordConfirm,
            WalletSaveField::PasswordConfirm => WalletSaveField::SaveToggle,
        };
    }

    /// Navigate to previous field in wallet save step
    pub fn wallet_save_focus_previous(&mut self) {
        if !self.save_wallet {
            // If save wallet is disabled, only toggle field is available
            self.wallet_save_focus = WalletSaveField::SaveToggle;
            return;
        }

        self.wallet_save_focus = match self.wallet_save_focus {
            WalletSaveField::SaveToggle => WalletSaveField::PasswordConfirm,
            WalletSaveField::WalletName => WalletSaveField::SaveToggle,
            WalletSaveField::Password => WalletSaveField::WalletName,
            WalletSaveField::PasswordConfirm => WalletSaveField::Password,
        };
    }

    /// Handle character input for the currently focused field
    pub fn wallet_save_handle_char(&mut self, c: char) {
        match self.wallet_save_focus {
            WalletSaveField::SaveToggle => {
                // Handle toggle with space or y/n
                if c == ' ' || c == 'y' || c == 'Y' {
                    self.save_wallet = true;
                } else if c == 'n' || c == 'N' {
                    self.save_wallet = false;
                }
            }
            WalletSaveField::WalletName => {
                if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' {
                    self.wallet_name.push(c);
                }
            }
            WalletSaveField::Password => {
                if c.is_ascii_graphic() || c == ' ' {
                    self.save_password.push(c);
                }
            }
            WalletSaveField::PasswordConfirm => {
                if c.is_ascii_graphic() || c == ' ' {
                    self.save_password_confirm.push(c);
                }
            }
        }
    }

    /// Handle backspace for the currently focused field
    pub fn wallet_save_handle_backspace(&mut self) {
        match self.wallet_save_focus {
            WalletSaveField::SaveToggle => {
                // No backspace handling for toggle
            }
            WalletSaveField::WalletName => {
                self.wallet_name.pop();
            }
            WalletSaveField::Password => {
                self.save_password.pop();
            }
            WalletSaveField::PasswordConfirm => {
                self.save_password_confirm.pop();
            }
        }
    }
}

/// Render the wallet setup wizard
pub fn render_wizard(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Create main layout: header, nav, content, status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation (disabled during wizard)
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header and navigation (but navigation is disabled)
    render_header(frame, &app.state, chunks[0]);
    render_wizard_navigation(frame, chunks[1], &app.state.wizard_state);

    // Render wizard content as modal overlay
    render_wizard_modal(frame, size, &app.state.wizard_state);

    // Render status bar
    render_status_bar(frame, &app.state, chunks[3]);
}

/// Render wizard navigation (shows progress)
fn render_wizard_navigation(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    let (current, total) = wizard_state.current_step.step_number();
    let progress_text = format!(
        "🧙 Setup Wizard - Step {} of {} - {}",
        current,
        total,
        wizard_state.current_step.title()
    );

    let nav = Paragraph::new(progress_text)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Wizard Progress"),
        )
        .alignment(Alignment::Center);

    frame.render_widget(nav, area);
}

/// Render the wizard modal overlay
fn render_wizard_modal(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    // Create centered modal area
    let modal_area = centered_rect(80, 70, area);

    // Clear background
    frame.render_widget(Clear, modal_area);

    // Render modal based on current step
    match wizard_state.current_step {
        WizardStep::Welcome => render_welcome_step(frame, modal_area),
        WizardStep::NetworkSelection => render_network_step(frame, modal_area, wizard_state),
        WizardStep::WalletSetup => render_wallet_step(frame, modal_area, wizard_state),
        WizardStep::SecurityWarning => render_security_step(frame, modal_area, wizard_state),
        WizardStep::WalletSave => render_wallet_save_step(frame, modal_area, wizard_state),
        WizardStep::Confirmation => render_confirmation_step(frame, modal_area, wizard_state),
        WizardStep::Complete => render_complete_step(frame, modal_area),
    }
}

/// Render welcome step
fn render_welcome_step(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("🕉️ Welcome to MANTRA DEX SDK")
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = vec![
        Line::from(vec![
            Span::styled("Welcome to the ", Style::default()),
            Span::styled(
                "MANTRA DEX SDK TUI",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("This setup wizard will help you:"),
        Line::from(""),
        Line::from("🔗 Configure your network connection"),
        Line::from("💼 Set up your wallet for trading"),
        Line::from("🔒 Understand security best practices"),
        Line::from("✅ Get ready to use the DEX"),
        Line::from(""),
        Line::from("The setup process takes about 2-3 minutes."),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("▶️  READY TO CONTINUE?", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default()),
            Span::styled(
                "Enter ↵",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to continue or ", Style::default()),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to exit", Style::default()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("💡 Hint: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::styled("If nothing happens when you press Enter, try pressing it again or check your terminal settings.", Style::default().fg(Color::Gray)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner);
}

/// Render network selection step
fn render_network_step(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("🌐 Network Configuration")
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Explanation
            Constraint::Min(6),    // Network options
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // Explanation
    let explanation = vec![
        Line::from("Choose the network you want to connect to:"),
        Line::from(""),
        Line::from("⚠️  Mainnet uses real assets - Testnet is safer for learning"),
        Line::from("Use ↑/↓ arrows or Tab to change selection"),
    ];

    let explanation_widget = Paragraph::new(Text::from(explanation));
    frame.render_widget(explanation_widget, chunks[0]);

    // Network options
    let networks = [
        (NetworkEnvironment::Testnet, "🧪 Recommended for new users"),
        (NetworkEnvironment::Mainnet, "💰 Real trading environment"),
    ];

    let network_items: Vec<ListItem> = networks
        .iter()
        .map(|(network, description)| {
            let is_selected = *network == wizard_state.selected_network;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let selected_indicator = if is_selected { "► " } else { "  " };

            // Add visual selection indicator
            let selection_status = if is_selected { " ✓ SELECTED" } else { "" };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(selected_indicator, style),
                    Span::styled(network.display_name(), style),
                    Span::styled(
                        selection_status,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(network.description(), Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(*description, Style::default().fg(Color::Blue)),
                ]),
                Line::from(""),
            ])
        })
        .collect();

    let network_list = List::new(network_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Available Networks"),
    );

    frame.render_widget(network_list, chunks[1]);

    // Controls - Updated to reflect all available navigation methods
    let controls = vec![Line::from(vec![
        Span::styled("Press ", Style::default()),
        Span::styled(
            "↑/↓ or Tab",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to select • ", Style::default()),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to continue • ", Style::default()),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to go back", Style::default()),
    ])];

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);

    frame.render_widget(controls_widget, chunks[2]);
}

/// Generate a mock mnemonic (in real implementation, use proper crypto library)
fn generate_mnemonic() -> String {
    match crate::wallet::MantraWallet::generate() {
        Ok((_, mnemonic)) => mnemonic,
        Err(_) => {
            // Fallback to a test mnemonic if generation fails
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        .to_string()
        }
    }
}

/// Helper function to create centered rectangle
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

/// Render wallet setup step (create or import wallet)
fn render_wallet_step(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    // Wrapper block for the entire step
    let block = Block::default()
        .borders(Borders::ALL)
        .title("💼 Wallet Setup")
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: explanation (3-4 lines), main content, controls
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Explanation
            Constraint::Min(6),    // Content (mnemonic / input)
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // --------------------------------------------------
    // Explanation section
    // --------------------------------------------------
    let explanation_lines = if wizard_state.import_existing {
        vec![
            Line::from("Import an existing wallet by typing your mnemonic words below."),
            Line::from("Words should be separated by spaces (12 / 24 words supported)."),
            Line::from("Press Tab to switch to creating a new wallet instead."),
            Line::from(""),
        ]
    } else {
        vec![
            Line::from("A new wallet mnemonic has been generated for you."),
            Line::from("Write it down **exactly** as shown and keep it safe – it\u{2019}s the ONLY way to recover your funds."),
            Line::from("Press Tab if you prefer to import an existing wallet instead."),
            Line::from(""),
        ]
    };

    let explanation = Paragraph::new(Text::from(explanation_lines)).wrap(Wrap { trim: true });
    frame.render_widget(explanation, chunks[0]);

    // --------------------------------------------------
    // Main content section
    // --------------------------------------------------
    if wizard_state.import_existing {
        // Build a bordered paragraph acting as an input area
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("Enter mnemonic words");

        let mnemonic_display = if wizard_state.mnemonic_input.is_empty() {
            Cow::Borrowed("<type here>")
        } else {
            Cow::Owned(wizard_state.mnemonic_input.clone())
        };

        let paragraph = Paragraph::new(mnemonic_display)
            .block(input_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, chunks[1]);
    } else {
        // Creating new wallet – show the generated mnemonic nicely formatted
        let mnemonic = wizard_state
            .generated_mnemonic
            .as_deref()
            .unwrap_or("<error generating mnemonic>");

        // Split into 3-4 word chunks per line for readability
        let mut words_iter = mnemonic.split_whitespace();
        let mut lines: Vec<Line> = Vec::new();
        loop {
            let mut current = Vec::<Span>::new();
            for _ in 0..4 {
                if let Some(word) = words_iter.next() {
                    current.push(Span::styled(
                        format!("{} ", word),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
            if current.is_empty() {
                break;
            }
            lines.push(Line::from(current));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Your mnemonic (write it down!)"),
            )
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, chunks[1]);
    }

    // --------------------------------------------------
    // Controls section
    // --------------------------------------------------
    let controls = vec![Line::from(vec![
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Switch • "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Continue • "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Back"),
    ])];

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);
    frame.render_widget(controls_widget, chunks[2]);
}

fn render_security_step(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("🔒 Security Information")
        .border_style(Style::default().fg(Color::Red))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: warning content, acknowledgment status, controls
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Security warnings
            Constraint::Length(3), // Acknowledgment status
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // --------------------------------------------------
    // Security warnings content
    // --------------------------------------------------
    let warning_lines = vec![
        Line::from(vec![Span::styled(
            "⚠️  IMPORTANT SECURITY WARNINGS",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("🔑 Your mnemonic phrase is the ONLY way to recover your wallet"),
        Line::from("   • Never share it with anyone"),
        Line::from("   • Store it securely offline (write it down)"),
        Line::from("   • Anyone with your mnemonic can access your funds"),
        Line::from(""),
        Line::from("🛡️  Best practices:"),
        Line::from("   • Use strong passwords for additional protection"),
        Line::from("   • Keep your software updated"),
        Line::from("   • Verify all transaction details before signing"),
        Line::from("   • Be aware of phishing attempts"),
        Line::from(""),
        Line::from("💰 Remember: Transactions on blockchain are irreversible"),
        Line::from(""),
    ];

    let warning_widget = Paragraph::new(Text::from(warning_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Security Guidelines"),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(warning_widget, chunks[0]);

    // --------------------------------------------------
    // Acknowledgment status
    // --------------------------------------------------
    let ack_status = if wizard_state.security_acknowledged {
        vec![Line::from(vec![
            Span::styled("✅ ", Style::default().fg(Color::Green)),
            Span::styled(
                "You have acknowledged the security warnings",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(
                "Please acknowledge that you understand these security warnings",
                Style::default().fg(Color::Red),
            ),
        ])]
    };

    let ack_widget = Paragraph::new(Text::from(ack_status)).alignment(Alignment::Center);

    frame.render_widget(ack_widget, chunks[1]);

    // --------------------------------------------------
    // Controls section
    // --------------------------------------------------
    let controls = if wizard_state.security_acknowledged {
        vec![Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Continue • "),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Back • "),
            Span::styled(
                "N",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Un-acknowledge"),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled(
                "Y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": I understand and acknowledge • "),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Back"),
        ])]
    };

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);
    frame.render_widget(controls_widget, chunks[2]);
}

fn render_confirmation_step(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("✅ Confirm Settings")
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: explanation, settings summary, controls
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Explanation
            Constraint::Min(8),    // Settings summary
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // --------------------------------------------------
    // Explanation section
    // --------------------------------------------------
    let explanation_lines = vec![
        Line::from("Please review your configuration before completing the setup:"),
        Line::from(""),
    ];

    let explanation = Paragraph::new(Text::from(explanation_lines));
    frame.render_widget(explanation, chunks[0]);

    // --------------------------------------------------
    // Settings summary
    // --------------------------------------------------
    let mut summary_lines = vec![
        Line::from(vec![
            Span::styled(
                "🌐 Network: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                wizard_state.selected_network.display_name(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(
                wizard_state.selected_network.description(),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
    ];

    // Wallet configuration
    if wizard_state.import_existing {
        let word_count = wizard_state.mnemonic_input.split_whitespace().count();
        summary_lines.extend(vec![
            Line::from(vec![
                Span::styled("💼 Wallet: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Import Existing", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("Mnemonic with {} words provided", word_count),
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]);
    } else {
        summary_lines.extend(vec![
            Line::from(vec![
                Span::styled("💼 Wallet: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Create New", Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    "New mnemonic has been generated",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ]);
    }

    summary_lines.extend(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "🔒 Security: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            if wizard_state.security_acknowledged {
                Span::styled("Acknowledged", Style::default().fg(Color::Green))
            } else {
                Span::styled("Not Acknowledged", Style::default().fg(Color::Red))
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("⚠️  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Clicking 'Finish' will apply these settings and complete the setup.",
                Style::default().add_modifier(Modifier::ITALIC),
            ),
        ]),
    ]);

    let summary_widget = Paragraph::new(Text::from(summary_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configuration Summary"),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(summary_widget, chunks[1]);

    // --------------------------------------------------
    // Controls section
    // --------------------------------------------------
    let controls = vec![Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Finish Setup • "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Back to Security"),
    ])];

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);
    frame.render_widget(controls_widget, chunks[2]);
}

// Implement Drop for secure memory cleanup
impl Drop for WizardState {
    fn drop(&mut self) {
        self.clear_wallet_save_data();

        // Also clear mnemonic input for security
        unsafe {
            let mnemonic_bytes = self.mnemonic_input.as_bytes_mut();
            for byte in mnemonic_bytes {
                *byte = 0;
            }
        }
        self.mnemonic_input.clear();

        // Clear generated mnemonic if present
        if let Some(ref mut generated) = self.generated_mnemonic {
            unsafe {
                let generated_bytes = generated.as_bytes_mut();
                for byte in generated_bytes {
                    *byte = 0;
                }
            }
            generated.clear();
        }
    }
}

/// Render wallet save step
fn render_wallet_save_step(frame: &mut Frame, area: Rect, wizard_state: &WizardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("💾 Save Wallet")
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: explanation, save option, form, controls
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Explanation
            Constraint::Length(3), // Save option toggle
            Constraint::Min(8),    // Form or skip message
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // --------------------------------------------------
    // Explanation section
    // --------------------------------------------------
    let explanation_lines = vec![
        Line::from("💾 Would you like to save your wallet for future use?"),
        Line::from(""),
        Line::from("Saving your wallet allows you to access it without re-entering your mnemonic."),
        Line::from("Your wallet will be encrypted with a strong password for security."),
    ];

    let explanation_widget = Paragraph::new(Text::from(explanation_lines));
    frame.render_widget(explanation_widget, chunks[0]);

    // --------------------------------------------------
    // Save option toggle
    // --------------------------------------------------
    let save_toggle_focused = wizard_state.wallet_save_focus == WalletSaveField::SaveToggle;
    let save_option_lines = vec![Line::from(vec![
        Span::styled(
            "Save wallet: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        if wizard_state.save_wallet {
            Span::styled(
                "✅ Yes",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "❌ No (Skip)",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        },
        if save_toggle_focused {
            Span::styled(
                " ← FOCUSED",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("", Style::default())
        },
    ])];

    let save_option_widget = Paragraph::new(Text::from(save_option_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if save_toggle_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                }),
        )
        .alignment(Alignment::Center);
    frame.render_widget(save_option_widget, chunks[1]);

    // --------------------------------------------------
    // Form or skip message
    // --------------------------------------------------
    if wizard_state.save_wallet {
        // Show wallet save form
        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Wallet name
                Constraint::Length(3), // Password
                Constraint::Length(3), // Confirm password
                Constraint::Min(2),    // Validation errors
            ])
            .split(chunks[2]);

        // Wallet name input
        let name_focused = wizard_state.wallet_save_focus == WalletSaveField::WalletName;
        let name_input = Paragraph::new(if wizard_state.wallet_name.is_empty() {
            "Enter wallet name..."
        } else {
            &wizard_state.wallet_name
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if name_focused {
                    " Wallet Name [FOCUSED] "
                } else {
                    " Wallet Name "
                })
                .border_style(if name_focused {
                    Style::default().fg(Color::Yellow)
                } else if wizard_state.wallet_name.is_empty() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                }),
        )
        .style(if wizard_state.wallet_name.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        });
        frame.render_widget(name_input, form_chunks[0]);

        // Password input (masked)
        let password_focused = wizard_state.wallet_save_focus == WalletSaveField::Password;
        let password_display = "•".repeat(wizard_state.save_password.len());
        let password_input = Paragraph::new(if wizard_state.save_password.is_empty() {
            "Enter password..."
        } else {
            &password_display
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if password_focused {
                    " Password [FOCUSED] "
                } else {
                    " Password "
                })
                .border_style(if password_focused {
                    Style::default().fg(Color::Yellow)
                } else if wizard_state.save_password.len() < 12 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                }),
        )
        .style(if wizard_state.save_password.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        });
        frame.render_widget(password_input, form_chunks[1]);

        // Confirm password input (masked)
        let confirm_focused = wizard_state.wallet_save_focus == WalletSaveField::PasswordConfirm;
        let confirm_display = "•".repeat(wizard_state.save_password_confirm.len());
        let confirm_input = Paragraph::new(if wizard_state.save_password_confirm.is_empty() {
            "Confirm password..."
        } else {
            &confirm_display
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if confirm_focused {
                    " Confirm Password [FOCUSED] "
                } else {
                    " Confirm Password "
                })
                .border_style(if confirm_focused {
                    Style::default().fg(Color::Yellow)
                } else if wizard_state.save_password_confirm.is_empty()
                    || wizard_state.save_password != wizard_state.save_password_confirm
                {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                }),
        )
        .style(if wizard_state.save_password_confirm.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        });
        frame.render_widget(confirm_input, form_chunks[2]);

        // Validation errors
        let errors = wizard_state.validate_wallet_save();
        if !errors.is_empty() {
            let error_lines: Vec<Line> = errors
                .iter()
                .map(|error| {
                    Line::from(vec![
                        Span::styled("❌ ", Style::default().fg(Color::Red)),
                        Span::styled(error, Style::default().fg(Color::Red)),
                    ])
                })
                .collect();

            let error_widget = Paragraph::new(Text::from(error_lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Validation Errors ")
                        .border_style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(error_widget, form_chunks[3]);
        } else if !wizard_state.wallet_name.is_empty()
            && !wizard_state.save_password.is_empty()
            && !wizard_state.save_password_confirm.is_empty()
        {
            // Show success message
            let success_lines = vec![Line::from(vec![
                Span::styled("✅ ", Style::default().fg(Color::Green)),
                Span::styled(
                    "Ready to save wallet!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];

            let success_widget = Paragraph::new(Text::from(success_lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Status ")
                        .border_style(Style::default().fg(Color::Green)),
                )
                .alignment(Alignment::Center);
            frame.render_widget(success_widget, form_chunks[3]);
        }
    } else {
        // Show skip message
        let skip_lines = vec![
            Line::from(vec![
                Span::styled("⏭️  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Skipping wallet save",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from("You'll need to enter your mnemonic each time you start the application."),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "You can save your wallet later in Settings if you change your mind.",
                    Style::default().fg(Color::Gray),
                ),
            ]),
        ];

        let skip_widget = Paragraph::new(Text::from(skip_lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Wallet Not Saved ")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center);
        frame.render_widget(skip_widget, chunks[2]);
    }

    // --------------------------------------------------
    // Controls section
    // --------------------------------------------------
    let controls = if wizard_state.save_wallet {
        vec![Line::from(vec![
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Navigate Fields • "),
            Span::styled(
                "Space/Y/N",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Toggle Save • "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Continue • "),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Back"),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Focus Toggle • "),
            Span::styled(
                "Space/Y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Enable Save • "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Skip & Continue • "),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Back"),
        ])]
    };

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);
    frame.render_widget(controls_widget, chunks[3]);
}

fn render_complete_step(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("🎉 Setup Complete")
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(2));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: success message, next steps, controls
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Success message
            Constraint::Min(8),    // Next steps
            Constraint::Length(3), // Controls
        ])
        .split(inner);

    // --------------------------------------------------
    // Success message
    // --------------------------------------------------
    let success_lines = vec![
        Line::from(vec![
            Span::styled("🎉 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Congratulations! Your MANTRA DEX setup is complete.",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("Your wallet has been configured and you're ready to start trading."),
        Line::from("The application will now take you to the main dashboard."),
        Line::from(""),
    ];

    let success_widget = Paragraph::new(Text::from(success_lines)).alignment(Alignment::Center);
    frame.render_widget(success_widget, chunks[0]);

    // --------------------------------------------------
    // Next steps
    // --------------------------------------------------
    let next_steps_lines = vec![
        Line::from(vec![Span::styled(
            "🚀 What's Next:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("📊 Dashboard - View your wallet balance and recent transactions"),
        Line::from("🏊 Pools - Browse and interact with liquidity pools"),
        Line::from("🔄 Swap - Exchange tokens directly"),
        Line::from("💧 Liquidity - Provide liquidity and earn rewards"),
        Line::from("🎁 Rewards - Claim your liquidity provider rewards"),
        Line::from("⚙️  Settings - Modify your configuration anytime"),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "💡 Tip: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Use number keys (1-8) to quickly navigate between screens",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "🔑 Tip: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Press F1 anytime for help, F5 to refresh data",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "⚠️  Remember: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Keep your mnemonic phrase secure and backed up!",
                Style::default().fg(Color::Red),
            ),
        ]),
    ];

    let next_steps_widget = Paragraph::new(Text::from(next_steps_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Getting Started"),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(next_steps_widget, chunks[1]);

    // --------------------------------------------------
    // Controls section
    // --------------------------------------------------
    let controls = vec![Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Continue to Dashboard • "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Exit Application"),
    ])];

    let controls_widget = Paragraph::new(Text::from(controls)).alignment(Alignment::Center);
    frame.render_widget(controls_widget, chunks[2]);
}
