//! Settings Screen Implementation
//!
//! This screen provides configuration options for network settings,
//! wallet management, and display preferences.

use crate::config::{Config, MantraNetworkConfig};
use crate::Error;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

/// Settings screen sections
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Network,
    Wallet,
    Display,
}

impl SettingsSection {
    /// Get display name for the section
    pub fn display_name(&self) -> &'static str {
        match self {
            SettingsSection::Network => "Network",
            SettingsSection::Wallet => "Wallet",
            SettingsSection::Display => "Display",
        }
    }

    /// Get all sections
    pub fn all() -> Vec<Self> {
        vec![
            SettingsSection::Network,
            SettingsSection::Wallet,
            SettingsSection::Display,
        ]
    }
}

/// Network environment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkEnvironment {
    Mainnet,
    Testnet,
    Custom,
}

impl NetworkEnvironment {
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            NetworkEnvironment::Mainnet => "Mainnet",
            NetworkEnvironment::Testnet => "Testnet",
            NetworkEnvironment::Custom => "Custom",
        }
    }

    /// Get all environments
    pub fn all() -> Vec<Self> {
        vec![
            NetworkEnvironment::Mainnet,
            NetworkEnvironment::Testnet,
            NetworkEnvironment::Custom,
        ]
    }
}

/// Theme options for display preferences
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Default,
    Dark,
    Light,
    HighContrast,
}

impl Theme {
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::Default => "Default",
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::HighContrast => "High Contrast",
        }
    }

    /// Get all themes
    pub fn all() -> Vec<Self> {
        vec![
            Theme::Default,
            Theme::Dark,
            Theme::Light,
            Theme::HighContrast,
        ]
    }
}

/// Simple form state for managing current field focus
#[derive(Debug, Clone)]
pub struct FormState {
    pub current_field: usize,
    pub editing: bool,
}

impl Default for FormState {
    fn default() -> Self {
        Self {
            current_field: 0,
            editing: false,
        }
    }
}

impl FormState {
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn handle_char(&mut self, _c: char) {
        // Basic character handling
    }
}

/// Simple input field structure
#[derive(Debug, Clone)]
pub struct InputField {
    pub label: String,
    pub value: String,
    pub is_sensitive: bool,
}

impl InputField {
    pub fn new(label: &str, default_value: &str, is_sensitive: bool) -> Self {
        Self {
            label: label.to_string(),
            value: default_value.to_string(),
            is_sensitive,
        }
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_string();
    }

    pub fn clear(&mut self) {
        self.value.clear();
    }

    pub fn handle_char(&mut self, c: char) {
        self.value.push(c);
    }

    pub fn handle_backspace(&mut self) {
        self.value.pop();
    }
}

/// Network configuration form state
#[derive(Debug, Clone)]
pub struct NetworkConfigForm {
    pub environment: NetworkEnvironment,
    pub custom_name: InputField,
    pub custom_rpc: InputField,
    pub gas_price: InputField,
    pub gas_adjustment: InputField,
    pub form_state: FormState,
}

impl Default for NetworkConfigForm {
    fn default() -> Self {
        Self {
            environment: NetworkEnvironment::Testnet,
            custom_name: InputField::new("Network Name", "mantra-dukong", false),
            custom_rpc: InputField::new(
                "RPC Endpoint",
                "https://rpc.dukong.mantrachain.io/",
                false,
            ),
            gas_price: InputField::new("Gas Price", "0.025", false),
            gas_adjustment: InputField::new("Gas Adjustment", "1.3", false),
            form_state: FormState::default(),
        }
    }
}

/// Wallet management form state
#[derive(Debug, Clone)]
pub struct WalletForm {
    pub mnemonic_input: InputField,
    pub form_state: FormState,
    pub show_mnemonic: bool,
    pub import_mode: bool,
}

impl Default for WalletForm {
    fn default() -> Self {
        Self {
            mnemonic_input: InputField::new("Mnemonic Phrase", "", true), // sensitive
            form_state: FormState::default(),
            show_mnemonic: false,
            import_mode: false,
        }
    }
}

/// Display preferences form state
#[derive(Debug, Clone)]
pub struct DisplayForm {
    pub theme: Theme,
    pub refresh_interval_balances: InputField,
    pub refresh_interval_pools: InputField,
    pub decimal_precision: InputField,
    pub auto_refresh: bool,
    pub form_state: FormState,
}

impl Default for DisplayForm {
    fn default() -> Self {
        Self {
            theme: Theme::Default,
            refresh_interval_balances: InputField::new("Balance Refresh (seconds)", "30", false),
            refresh_interval_pools: InputField::new("Pool Refresh (seconds)", "60", false),
            decimal_precision: InputField::new("Decimal Precision", "6", false),
            auto_refresh: true,
            form_state: FormState::default(),
        }
    }
}

/// Settings screen state
#[derive(Debug, Clone)]
pub struct SettingsState {
    /// Current settings section
    pub current_section: SettingsSection,
    /// Section list state for navigation
    pub section_list_state: ListState,
    /// Network configuration form
    pub network_form: NetworkConfigForm,
    /// Wallet management form
    pub wallet_form: WalletForm,
    /// Display preferences form
    pub display_form: DisplayForm,
    /// Current configuration
    pub current_config: Config,
    /// Whether changes have been made
    pub has_changes: bool,
    /// Confirmation modal state
    pub show_confirmation: bool,
    /// Success/error messages
    pub message: Option<(String, bool)>, // (message, is_error)
}

impl Default for SettingsState {
    fn default() -> Self {
        let mut state = Self {
            current_section: SettingsSection::Network,
            section_list_state: ListState::default(),
            network_form: NetworkConfigForm::default(),
            wallet_form: WalletForm::default(),
            display_form: DisplayForm::default(),
            current_config: Config::default(),
            has_changes: false,
            show_confirmation: false,
            message: None,
        };

        // Select the first section by default
        state.section_list_state.select(Some(0));
        state
    }
}

impl SettingsState {
    /// Create new settings state with existing config
    pub fn new(config: Config) -> Self {
        let mut state = Self::default();
        state.current_config = config.clone();
        state.load_config_into_forms(&config);
        state
    }

    /// Load configuration values into forms
    pub fn load_config_into_forms(&mut self, config: &Config) {
        // Load network config
        self.network_form
            .custom_name
            .set_value(&config.network.network_name);
        self.network_form
            .custom_rpc
            .set_value(&config.network.rpc_url);
        self.network_form
            .gas_price
            .set_value(&config.network.gas_price.to_string());
        self.network_form
            .gas_adjustment
            .set_value(&config.network.gas_adjustment.to_string());

        // Determine environment based on network name
        self.network_form.environment = if config.network.network_name.contains("mainnet") {
            NetworkEnvironment::Mainnet
        } else if config.network.network_name.contains("testnet")
            || config.network.network_name.contains("dukong")
        {
            NetworkEnvironment::Testnet
        } else {
            NetworkEnvironment::Custom
        };

        // Load wallet config (but don't show mnemonic for security)
        if config.mnemonic.is_some() {
            self.wallet_form
                .mnemonic_input
                .set_value("*** MNEMONIC SET ***");
        }
    }

    /// Navigate to next section
    pub fn next_section(&mut self) {
        let sections = SettingsSection::all();
        let current_index = sections
            .iter()
            .position(|&s| s == self.current_section)
            .unwrap_or(0);
        let next_index = (current_index + 1) % sections.len();
        self.current_section = sections[next_index];
        self.section_list_state.select(Some(next_index));
    }

    /// Navigate to previous section
    pub fn previous_section(&mut self) {
        let sections = SettingsSection::all();
        let current_index = sections
            .iter()
            .position(|&s| s == self.current_section)
            .unwrap_or(0);
        let prev_index = if current_index == 0 {
            sections.len() - 1
        } else {
            current_index - 1
        };
        self.current_section = sections[prev_index];
        self.section_list_state.select(Some(prev_index));
    }

    /// Handle character input
    pub fn handle_char_input(&mut self, c: char) -> Result<(), Error> {
        match self.current_section {
            SettingsSection::Network => {
                if self.network_form.form_state.is_editing() {
                    self.network_form.form_state.handle_char(c);

                    // Update the appropriate field based on current focus
                    match self.network_form.form_state.current_field {
                        0 => self.network_form.custom_name.handle_char(c),
                        1 => self.network_form.custom_rpc.handle_char(c),
                        2 => self.network_form.gas_price.handle_char(c),
                        3 => self.network_form.gas_adjustment.handle_char(c),
                        _ => {}
                    }
                    self.has_changes = true;
                }
            }
            SettingsSection::Wallet => {
                if self.wallet_form.form_state.is_editing() && self.wallet_form.import_mode {
                    self.wallet_form.mnemonic_input.handle_char(c);
                    self.has_changes = true;
                }
            }
            SettingsSection::Display => {
                if self.display_form.form_state.is_editing() {
                    match self.display_form.form_state.current_field {
                        0 => self.display_form.refresh_interval_balances.handle_char(c),
                        1 => self.display_form.refresh_interval_pools.handle_char(c),
                        2 => self.display_form.decimal_precision.handle_char(c),
                        _ => {}
                    }
                    self.has_changes = true;
                }
            }
        }
        Ok(())
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) -> Result<(), Error> {
        match self.current_section {
            SettingsSection::Network => {
                if self.network_form.form_state.is_editing() {
                    match self.network_form.form_state.current_field {
                        0 => self.network_form.custom_name.handle_backspace(),
                        1 => self.network_form.custom_rpc.handle_backspace(),
                        2 => self.network_form.gas_price.handle_backspace(),
                        3 => self.network_form.gas_adjustment.handle_backspace(),
                        _ => {}
                    }
                    self.has_changes = true;
                }
            }
            SettingsSection::Wallet => {
                if self.wallet_form.form_state.is_editing() && self.wallet_form.import_mode {
                    self.wallet_form.mnemonic_input.handle_backspace();
                    self.has_changes = true;
                }
            }
            SettingsSection::Display => {
                if self.display_form.form_state.is_editing() {
                    match self.display_form.form_state.current_field {
                        0 => self
                            .display_form
                            .refresh_interval_balances
                            .handle_backspace(),
                        1 => self.display_form.refresh_interval_pools.handle_backspace(),
                        2 => self.display_form.decimal_precision.handle_backspace(),
                        _ => {}
                    }
                    self.has_changes = true;
                }
            }
        }
        Ok(())
    }

    /// Toggle network environment
    pub fn toggle_network_environment(&mut self) {
        let environments = NetworkEnvironment::all();
        let current_index = environments
            .iter()
            .position(|&e| e == self.network_form.environment)
            .unwrap_or(0);
        let next_index = (current_index + 1) % environments.len();
        self.network_form.environment = environments[next_index];
        self.has_changes = true;
    }

    /// Toggle theme
    pub fn toggle_theme(&mut self) {
        let themes = Theme::all();
        let current_index = themes
            .iter()
            .position(|&t| t == self.display_form.theme)
            .unwrap_or(0);
        let next_index = (current_index + 1) % themes.len();
        self.display_form.theme = themes[next_index];
        self.has_changes = true;
    }

    /// Toggle auto refresh
    pub fn toggle_auto_refresh(&mut self) {
        self.display_form.auto_refresh = !self.display_form.auto_refresh;
        self.has_changes = true;
    }

    /// Toggle wallet import mode
    pub fn toggle_import_mode(&mut self) {
        self.wallet_form.import_mode = !self.wallet_form.import_mode;
        if self.wallet_form.import_mode {
            self.wallet_form.mnemonic_input.clear();
        }
    }

    /// Toggle mnemonic visibility
    pub fn toggle_mnemonic_visibility(&mut self) {
        self.wallet_form.show_mnemonic = !self.wallet_form.show_mnemonic;
    }

    /// Save current settings
    pub fn save_settings(&mut self) -> Result<Config, Error> {
        let mut new_config = self.current_config.clone();

        // Update network configuration
        match self.network_form.environment {
            NetworkEnvironment::Mainnet => {
                new_config.network = MantraNetworkConfig {
                    network_name: "mantra-mainnet".to_string(),
                    chain_id: "mantra-mainnet-1".to_string(),
                    rpc_url: "https://rpc.mantrachain.io/".to_string(),
                    gas_price: 0.025,
                    gas_adjustment: 1.3,
                    native_denom: "uom".to_string(),
                    contracts: new_config.network.contracts.clone(),
                };
            }
            NetworkEnvironment::Testnet => {
                new_config.network = MantraNetworkConfig {
                    network_name: "mantra-dukong".to_string(),
                    chain_id: "mantra-dukong-1".to_string(),
                    rpc_url: "https://rpc.dukong.mantrachain.io/".to_string(),
                    gas_price: 0.025,
                    gas_adjustment: 1.3,
                    native_denom: "uom".to_string(),
                    contracts: new_config.network.contracts.clone(),
                };
            }
            NetworkEnvironment::Custom => {
                new_config.network.network_name = self.network_form.custom_name.value.clone();
                new_config.network.rpc_url = self.network_form.custom_rpc.value.clone();

                // Parse gas price and adjustment
                if let Ok(gas_price) = self.network_form.gas_price.value.parse::<f64>() {
                    new_config.network.gas_price = gas_price;
                }
                if let Ok(gas_adj) = self.network_form.gas_adjustment.value.parse::<f64>() {
                    new_config.network.gas_adjustment = gas_adj;
                }
            }
        }

        // Update wallet configuration if in import mode and mnemonic is provided
        if self.wallet_form.import_mode
            && !self.wallet_form.mnemonic_input.value.is_empty()
            && self.wallet_form.mnemonic_input.value != "*** MNEMONIC SET ***"
        {
            new_config.mnemonic = Some(self.wallet_form.mnemonic_input.value.clone());
        }

        // Save to file
        let config_path = Config::default_path();
        new_config.save(&config_path)?;

        self.current_config = new_config.clone();
        self.has_changes = false;
        self.message = Some(("Settings saved successfully!".to_string(), false));

        Ok(new_config)
    }

    /// Reset to defaults
    pub fn reset_to_defaults(&mut self) {
        *self = SettingsState::default();
        self.message = Some(("Settings reset to defaults".to_string(), false));
    }

    /// Clear messages
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Move focus to next field in current section
    pub fn next_field(&mut self) {
        match self.current_section {
            SettingsSection::Network => {
                self.network_form.form_state.current_field =
                    (self.network_form.form_state.current_field + 1) % 4; // 4 fields in network section
            }
            SettingsSection::Wallet => {
                self.wallet_form.form_state.current_field =
                    (self.wallet_form.form_state.current_field + 1) % 1; // 1 field in wallet section
            }
            SettingsSection::Display => {
                self.display_form.form_state.current_field =
                    (self.display_form.form_state.current_field + 1) % 3; // 3 fields in display section
            }
        }
    }

    /// Move focus to previous field in current section
    pub fn previous_field(&mut self) {
        match self.current_section {
            SettingsSection::Network => {
                if self.network_form.form_state.current_field == 0 {
                    self.network_form.form_state.current_field = 3; // wrap to last field
                } else {
                    self.network_form.form_state.current_field -= 1;
                }
            }
            SettingsSection::Wallet => {
                // Only one field, no change needed
            }
            SettingsSection::Display => {
                if self.display_form.form_state.current_field == 0 {
                    self.display_form.form_state.current_field = 2; // wrap to last field
                } else {
                    self.display_form.form_state.current_field -= 1;
                }
            }
        }
    }

    /// Check if currently focused field is editable
    pub fn is_current_field_editable(&self) -> bool {
        match self.current_section {
            SettingsSection::Network => {
                // Network fields are editable when custom environment is selected
                matches!(self.network_form.environment, NetworkEnvironment::Custom)
            }
            SettingsSection::Wallet => {
                // Wallet field is editable when in import mode
                self.wallet_form.import_mode
            }
            SettingsSection::Display => {
                // Display fields are always editable
                true
            }
        }
    }

    /// Get the current field name for focus identification
    pub fn get_current_field_id(&self) -> Option<String> {
        match self.current_section {
            SettingsSection::Network => match self.network_form.form_state.current_field {
                0 => Some("settings_network_name".to_string()),
                1 => Some("settings_network_rpc".to_string()),
                2 => Some("settings_gas_price".to_string()),
                3 => Some("settings_gas_adjustment".to_string()),
                _ => None,
            },
            SettingsSection::Wallet => Some("settings_wallet_mnemonic".to_string()),
            SettingsSection::Display => match self.display_form.form_state.current_field {
                0 => Some("settings_balance_refresh".to_string()),
                1 => Some("settings_pool_refresh".to_string()),
                2 => Some("settings_decimal_precision".to_string()),
                _ => None,
            },
        }
    }
}

/// Render the settings screen with standard layout
pub fn render_settings_screen(frame: &mut Frame, app: &crate::tui_dex::app::App) {
    let size = frame.area();

    // Create main layout: header, navigation, content, status
    let main_chunks = ratatui::layout::Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header, navigation, and status bar
    crate::tui_dex::components::header::render_header(frame, &app.state, main_chunks[0]);
    crate::tui_dex::components::navigation::render_navigation(frame, &app.state, main_chunks[1]);
    crate::tui_dex::components::status_bar::render_status_bar(frame, &app.state, main_chunks[3]);

    // Render settings content
    render_settings(frame, main_chunks[2], &mut app.state.settings_state.clone());
}

/// Render the settings screen
pub fn render_settings(frame: &mut Frame, area: Rect, state: &mut SettingsState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    // Render section navigation
    render_section_navigation(frame, chunks[0], state);

    // Render current section content
    match state.current_section {
        SettingsSection::Network => render_network_settings(frame, chunks[1], state),
        SettingsSection::Wallet => render_wallet_settings(frame, chunks[1], state),
        SettingsSection::Display => render_display_settings(frame, chunks[1], state),
    }

    // Render confirmation modal if needed
    if state.show_confirmation {
        render_confirmation_modal(frame, area, state);
    }

    // Render message if any
    if let Some((message, is_error)) = &state.message {
        render_message_modal(frame, area, message, *is_error);
    }
}

/// Render section navigation panel
fn render_section_navigation(frame: &mut Frame, area: Rect, state: &mut SettingsState) {
    let sections = SettingsSection::all();
    let items: Vec<ListItem> = sections
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let is_selected = Some(i) == state.section_list_state.selected();
            let is_current = *section == state.current_section;

            let style = if is_current && is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_current { "→ " } else { "  " };
            ListItem::new(format!("{}{}", prefix, section.display_name())).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Settings Sections")
                .border_style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut state.section_list_state);

    // Add enhanced help text at the bottom
    let help_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(5),
        width: area.width,
        height: 5,
    };

    let help_text = Paragraph::new(
        "Navigation:\n↑/↓: Change Section\nEnter: Select\n→/←: Move Focus\nSpace: Toggle",
    )
    .block(Block::default().borders(Borders::TOP))
    .style(Style::default().fg(Color::DarkGray))
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: true });

    frame.render_widget(help_text, help_area);
}

/// Render network configuration settings
fn render_network_settings(frame: &mut Frame, area: Rect, state: &mut SettingsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Environment selection
            Constraint::Min(10),   // Form fields
            Constraint::Length(3), // Actions
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Network Configuration")
        .block(Block::default().borders(Borders::ALL))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Environment selection - check if focused by global focus manager
    let env_text = format!(
        "Environment: {} (Press 'e' to toggle)",
        state.network_form.environment.display_name()
    );
    let env_paragraph = Paragraph::new(env_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Network Environment"),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(env_paragraph, chunks[1]);

    // Form fields (only show for custom environment)
    if state.network_form.environment == NetworkEnvironment::Custom {
        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(chunks[2]);

        // Render input fields - note: these will be rendered by global focus system later
        render_input_field(
            frame,
            form_chunks[0],
            &state.network_form.custom_name,
            state.network_form.form_state.current_field == 0
                && state.network_form.form_state.is_editing(),
        );
        render_input_field(
            frame,
            form_chunks[1],
            &state.network_form.custom_rpc,
            state.network_form.form_state.current_field == 1
                && state.network_form.form_state.is_editing(),
        );
        render_input_field(
            frame,
            form_chunks[2],
            &state.network_form.gas_price,
            state.network_form.form_state.current_field == 2
                && state.network_form.form_state.is_editing(),
        );
        render_input_field(
            frame,
            form_chunks[3],
            &state.network_form.gas_adjustment,
            state.network_form.form_state.current_field == 3
                && state.network_form.form_state.is_editing(),
        );
    } else {
        // Show current network info for mainnet/testnet
        let network_info = match state.network_form.environment {
            NetworkEnvironment::Mainnet => {
                vec![
                    "Network: Mantra Mainnet",
                    "RPC: https://rpc.mantrachain.io/",
                    "Gas Price: 0.025 uom",
                    "Gas Adjustment: 1.3",
                ]
            }
            NetworkEnvironment::Testnet => {
                vec![
                    "Network: Mantra Dukong Testnet",
                    "RPC: https://rpc.dukong.mantrachain.io/",
                    "Gas Price: 0.025 uom",
                    "Gas Adjustment: 1.3",
                ]
            }
            NetworkEnvironment::Custom => vec![], // Should not reach here
        };

        let info_text = network_info.join("\n");
        let info_paragraph = Paragraph::new(info_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Network Information"),
            )
            .style(Style::default().fg(Color::Green))
            .wrap(Wrap { trim: true });
        frame.render_widget(info_paragraph, chunks[2]);
    }

    // Actions
    let actions_text = if state.has_changes {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | Tab: Navigate | Changes pending..."
    } else {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | Tab: Navigate Fields"
    };

    let actions_style = if state.has_changes {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let actions_paragraph = Paragraph::new(actions_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keyboard Shortcuts"),
        )
        .style(actions_style)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(actions_paragraph, chunks[3]);
}

/// Render wallet management settings
fn render_wallet_settings(frame: &mut Frame, area: Rect, state: &mut SettingsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Current wallet info
            Constraint::Length(5), // Import mode toggle
            Constraint::Min(5),    // Mnemonic input
            Constraint::Length(3), // Actions
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Wallet Management")
        .block(Block::default().borders(Borders::ALL))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Current wallet info
    let wallet_info = if state.current_config.mnemonic.is_some() {
        "Status: Wallet configured\nAddress: Loaded from mnemonic\nNote: Wallet will be used for all transactions"
    } else {
        "Status: No wallet configured\nAddress: None\nNote: You need to import a mnemonic to use the DEX"
    };

    let wallet_style = if state.current_config.mnemonic.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let wallet_paragraph = Paragraph::new(wallet_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Current Wallet"),
        )
        .style(wallet_style)
        .wrap(Wrap { trim: true });
    frame.render_widget(wallet_paragraph, chunks[1]);

    // Import mode toggle
    let import_text = format!(
        "Import Mode: {} (Press 'i' to toggle)",
        if state.wallet_form.import_mode {
            "ON"
        } else {
            "OFF"
        }
    );
    let import_paragraph = Paragraph::new(import_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Import New Wallet"),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(import_paragraph, chunks[2]);

    // Mnemonic input (only show if in import mode)
    if state.wallet_form.import_mode {
        let mnemonic_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(2)])
            .split(chunks[3]);

        render_input_field(
            frame,
            mnemonic_chunks[0],
            &state.wallet_form.mnemonic_input,
            state.wallet_form.form_state.is_editing(),
        );

        let help_text = "Enter your 12 or 24 word mnemonic phrase\nWarning: This will replace your current wallet!";
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(help_paragraph, mnemonic_chunks[1]);
    } else {
        // Show export/backup options
        let export_text = vec![
            "Backup Options:",
            "• Export wallet address: Ctrl+E",
            "• View mnemonic phrase: Ctrl+M (requires confirmation)",
            "",
            "Security Note: Never share your mnemonic phrase!",
        ]
        .join("\n");

        let export_paragraph = Paragraph::new(export_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Backup & Export"),
            )
            .style(Style::default().fg(Color::Blue))
            .wrap(Wrap { trim: true });
        frame.render_widget(export_paragraph, chunks[3]);
    }

    // Actions
    let actions_text = if state.has_changes {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | Tab: Navigate | Changes pending..."
    } else {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | i: Import | m: Show Mnemonic"
    };

    let actions_style = if state.has_changes {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let actions_paragraph = Paragraph::new(actions_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keyboard Shortcuts"),
        )
        .style(actions_style)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(actions_paragraph, chunks[4]);
}

/// Render display preferences settings
fn render_display_settings(frame: &mut Frame, area: Rect, state: &mut SettingsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Theme selection
            Constraint::Length(5), // Auto-refresh toggle
            Constraint::Min(5),    // Form fields
            Constraint::Length(3), // Actions
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Display Preferences")
        .block(Block::default().borders(Borders::ALL))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Theme selection
    let theme_text = format!(
        "Theme: {} (Press 't' to toggle)",
        state.display_form.theme.display_name()
    );
    let theme_paragraph = Paragraph::new(theme_text)
        .block(Block::default().borders(Borders::ALL).title("Color Theme"))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(theme_paragraph, chunks[1]);

    // Auto-refresh toggle
    let refresh_text = format!(
        "Auto-refresh: {} (Press 'a' to toggle)",
        if state.display_form.auto_refresh {
            "ON"
        } else {
            "OFF"
        }
    );
    let refresh_paragraph = Paragraph::new(refresh_text)
        .block(Block::default().borders(Borders::ALL).title("Data Refresh"))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(refresh_paragraph, chunks[2]);

    // Form fields for intervals and precision
    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(chunks[3]);

    render_input_field(
        frame,
        form_chunks[0],
        &state.display_form.refresh_interval_balances,
        state.display_form.form_state.current_field == 0
            && state.display_form.form_state.is_editing(),
    );
    render_input_field(
        frame,
        form_chunks[1],
        &state.display_form.refresh_interval_pools,
        state.display_form.form_state.current_field == 1
            && state.display_form.form_state.is_editing(),
    );
    render_input_field(
        frame,
        form_chunks[2],
        &state.display_form.decimal_precision,
        state.display_form.form_state.current_field == 2
            && state.display_form.form_state.is_editing(),
    );

    // Actions
    let actions_text = if state.has_changes {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | Tab: Navigate | Changes pending..."
    } else {
        "Actions: Ctrl+S: Save Changes | Ctrl+R: Reset | t: Theme | a: Auto-refresh"
    };

    let actions_style = if state.has_changes {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let actions_paragraph = Paragraph::new(actions_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keyboard Shortcuts"),
        )
        .style(actions_style)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(actions_paragraph, chunks[4]);
}

/// Render input field helper
fn render_input_field(frame: &mut Frame, area: Rect, field: &InputField, is_focused: bool) {
    let style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let display_value = if field.is_sensitive && !field.value.is_empty() {
        "*".repeat(field.value.len())
    } else {
        field.value.clone()
    };

    let input_text = if is_focused {
        format!("{}_", display_value)
    } else {
        display_value
    };

    let paragraph = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(field.label.as_str())
                .border_style(border_style),
        )
        .style(style);

    frame.render_widget(paragraph, area);
}

/// Render confirmation modal
fn render_confirmation_modal(frame: &mut Frame, area: Rect, _state: &SettingsState) {
    let popup_area = centered_rect(50, 30, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Confirm Changes")
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(block, popup_area);

    let inner_area = popup_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let text =
        "Are you sure you want to save these changes?\n\nPress Enter to confirm, Esc to cancel";
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner_area);
}

/// Render message modal
fn render_message_modal(frame: &mut Frame, area: Rect, message: &str, is_error: bool) {
    let popup_area = centered_rect(50, 20, area);

    frame.render_widget(Clear, popup_area);

    let title = if is_error { "Error" } else { "Success" };
    let style = if is_error { Color::Red } else { Color::Green };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(style));

    frame.render_widget(block, popup_area);

    let inner_area = popup_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let paragraph = Paragraph::new(format!("{}\n\nPress any key to continue", message))
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner_area);
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

/// Enhanced render settings screen that integrates with global focus manager
pub fn render_settings_screen_with_focus(frame: &mut Frame, app: &crate::tui_dex::app::App) {
    // Use the existing settings rendering
    let size = frame.area();

    // Create main layout: header, nav, content, status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Import the necessary components
    use crate::tui_dex::components::{
        header::render_header, navigation::render_navigation, status_bar::render_status_bar,
    };

    // Render header and navigation
    render_header(frame, &app.state, chunks[0]);
    render_navigation(frame, &app.state, chunks[1]);

    // Render settings content
    render_settings(frame, chunks[2], &mut app.state.settings_state.clone());

    // Render global focus indicators when in content mode
    if app.state.navigation_mode == crate::tui_dex::app::NavigationMode::WithinScreen {
        render_settings_focus_indicators(frame, chunks[2], app);
    }

    // Render status bar
    render_status_bar(frame, &app.state, chunks[3]);
}

/// Render focus indicators for settings elements using global focus manager
fn render_settings_focus_indicators(frame: &mut Frame, area: Rect, app: &crate::tui_dex::app::App) {
    if let Some(focused) = app.state.focus_manager.current_focus() {
        match focused {
            crate::tui_dex::events::FocusableComponent::Dropdown(dropdown_id) => {
                if dropdown_id == "settings_network" {
                    // Highlight the network dropdown area
                    let network_area = Rect {
                        x: area.x + area.width / 3,
                        y: area.y + 8,
                        width: area.width * 2 / 3 - 2,
                        height: 5,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .title("[ FOCUSED: NETWORK SETTINGS ]");

                    frame.render_widget(highlight, network_area);
                }
            }
            crate::tui_dex::events::FocusableComponent::TextInput(input_id) => {
                if input_id == "settings_rpc" {
                    // Highlight the RPC input area
                    let rpc_area = Rect {
                        x: area.x + area.width / 3,
                        y: area.y + 14,
                        width: area.width * 2 / 3 - 2,
                        height: 3,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .title("[ FOCUSED: RPC ENDPOINT ]");

                    frame.render_widget(highlight, rpc_area);
                } else if input_id == "settings_wallet" {
                    // Highlight the wallet input area
                    let wallet_area = Rect {
                        x: area.x + area.width / 3,
                        y: area.y + 18,
                        width: area.width * 2 / 3 - 2,
                        height: 3,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .title("[ FOCUSED: WALLET SETTINGS ]");

                    frame.render_widget(highlight, wallet_area);
                }
            }
            crate::tui_dex::events::FocusableComponent::Button(button_id) => {
                if button_id == "settings_save" {
                    // Highlight the save button area
                    let save_area = Rect {
                        x: area.x + area.width / 3,
                        y: area.y + area.height - 8,
                        width: 15,
                        height: 3,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                        .title("[ SAVE ]");

                    frame.render_widget(highlight, save_area);
                } else if button_id == "settings_reset" {
                    // Highlight the reset button area
                    let reset_area = Rect {
                        x: area.x + area.width / 3 + 16,
                        y: area.y + area.height - 8,
                        width: 15,
                        height: 3,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                        .title("[ RESET ]");

                    frame.render_widget(highlight, reset_area);
                }
            }
            _ => {}
        }
    }
}

/// Settings screen implementation for MANTRA DEX SDK TUI
pub struct SettingsScreen;

impl SettingsScreen {
    pub fn new() -> Self {
        Self
    }
}

/// Initialize settings screen focus management
pub fn initialize_settings_screen_focus() {
    // This function can be called when entering the settings screen
    // to ensure proper focus initialization
}

/// Settings screen focus management helper
pub fn get_focusable_components_for_section(section: SettingsSection) -> Vec<String> {
    match section {
        SettingsSection::Network => vec![
            "settings_network_environment".to_string(),
            "settings_network_name".to_string(),
            "settings_network_rpc".to_string(),
            "settings_gas_price".to_string(),
            "settings_gas_adjustment".to_string(),
        ],
        SettingsSection::Wallet => vec![
            "settings_wallet_import_mode".to_string(),
            "settings_wallet_mnemonic".to_string(),
            "settings_wallet_show_mnemonic".to_string(),
        ],
        SettingsSection::Display => vec![
            "settings_theme".to_string(),
            "settings_balance_refresh".to_string(),
            "settings_pool_refresh".to_string(),
            "settings_decimal_precision".to_string(),
            "settings_auto_refresh".to_string(),
        ],
    }
}
