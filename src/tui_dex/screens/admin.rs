//! Admin Screen Implementation
//!
//! This module provides the admin view for the MANTRA DEX SDK TUI,
//! allowing pool creation, feature management, and administrative operations.

use crate::tui_dex::{
    app::{App, LoadingState},
    components::{
        forms::{InputType, TextInput},
        header::render_header,
        navigation::render_navigation,
        simple_list::{ListEvent, SimpleList, SimpleListOption},
        status_bar::render_status_bar,
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, Padding, Paragraph, Tabs, Wrap},
    Frame,
};
use tui_input::InputRequest;

/// Admin screen operational modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminMode {
    PoolManagement,
    PoolCreation,
    FeatureControls,
}

/// Input focus states for the admin screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdminInputFocus {
    // Pool Management
    PoolSelection,
    FeatureToggles,
    ManagementExecute,

    // Pool Creation
    FirstAssetDenom,
    SecondAssetDenom,
    SwapFee,
    ProtocolFee,
    BurnFee,
    PoolType,
    CreationExecute,

    // Feature Controls
    TargetPoolId,
    FeatureControls,
    ControlsExecute,
}

/// Validation result containing both boolean status and error messages
#[derive(Debug, Clone)]
struct ValidationResult {
    is_valid: bool,
    errors: Vec<String>,
}

/// Pool creation form state (simplified like swap/liquidity screens)
#[derive(Debug, Clone)]
pub struct PoolCreationState {
    pub first_asset_input: TextInput,
    pub second_asset_input: TextInput,
    pub swap_fee_input: TextInput,
    pub protocol_fee_input: TextInput,
    pub burn_fee_input: TextInput,
    pub pool_type_dropdown: SimpleList,
    pub validation_errors: Vec<String>,
}

impl Default for PoolCreationState {
    fn default() -> Self {
        let first_asset_input = TextInput::new("First Asset Denomination")
            .required()
            .with_placeholder("e.g., uom");

        let second_asset_input = TextInput::new("Second Asset Denomination")
            .required()
            .with_placeholder("e.g., factory/contract/token");

        let swap_fee_input = TextInput::new("Swap Fee (%)")
            .with_type(InputType::Amount)
            .with_value("0.03")
            .with_placeholder("0.03");

        let protocol_fee_input = TextInput::new("Protocol Fee (%)")
            .with_type(InputType::Amount)
            .with_value("0.01")
            .with_placeholder("0.01");

        let burn_fee_input = TextInput::new("Burn Fee (%)")
            .with_type(InputType::Amount)
            .with_value("0.0")
            .with_placeholder("0.0");

        let pool_type_options = vec![
            SimpleListOption::new(
                "Constant Product".to_string(),
                "ConstantProduct".to_string(),
            ),
            SimpleListOption::new("Stable Swap".to_string(), "StableSwap".to_string()),
        ];
        let mut pool_type_dropdown = SimpleList::new("Pool Type").with_options(pool_type_options);
        // Set initial selection to first option (Constant Product)
        pool_type_dropdown.selected_index = Some(0);

        Self {
            first_asset_input,
            second_asset_input,
            swap_fee_input,
            protocol_fee_input,
            burn_fee_input,
            pool_type_dropdown,
            validation_errors: Vec::new(),
        }
    }
}

/// Pool feature control state (simplified like swap/liquidity screens)
#[derive(Debug, Clone)]
pub struct PoolFeatureState {
    pub pool_selection_dropdown: SimpleList,
    pub withdrawals_enabled: bool,
    pub deposits_enabled: bool,
    pub swaps_enabled: bool,
}

impl Default for PoolFeatureState {
    fn default() -> Self {
        let pool_selection_dropdown = SimpleList::new("Select Pool to Manage");

        Self {
            pool_selection_dropdown,
            withdrawals_enabled: true,
            deposits_enabled: true,
            swaps_enabled: true,
        }
    }
}

/// Pool management state
#[derive(Debug, Clone)]
pub struct PoolManagementState {
    pub pool_selection_dropdown: SimpleList,
    pub selected_pool_features: Option<(bool, bool, bool)>, // (withdrawals, deposits, swaps)
}

impl Default for PoolManagementState {
    fn default() -> Self {
        let pool_selection_dropdown = SimpleList::new("Select Pool");

        Self {
            pool_selection_dropdown,
            selected_pool_features: None,
        }
    }
}

/// Admin screen state management (simplified like swap/liquidity screens)
#[derive(Debug, Clone)]
pub struct AdminScreenState {
    /// Current mode/tab
    pub mode: AdminMode,
    /// Current input focus
    pub input_focus: AdminInputFocus,
    /// Pool management state
    pub pool_management: PoolManagementState,
    /// Pool creation state
    pub pool_creation: PoolCreationState,
    /// Feature control state
    pub feature_control: PoolFeatureState,
    /// Available pools for management
    pub available_pools: Vec<(String, String)>, // (pool_id, display_name)
    /// Timer for input changes
    pub last_input_change: Option<std::time::Instant>,
}

impl Default for AdminScreenState {
    fn default() -> Self {
        let mut instance = Self {
            mode: AdminMode::PoolManagement,
            input_focus: AdminInputFocus::PoolSelection,
            pool_management: PoolManagementState::default(),
            pool_creation: PoolCreationState::default(),
            feature_control: PoolFeatureState::default(),
            available_pools: Vec::new(),
            last_input_change: None,
        };

        // Apply initial focus
        instance.apply_focus();
        instance
    }
}

impl AdminScreenState {
    /// Update available pools for admin operations
    pub fn update_available_pools(&mut self, pools: Vec<(String, String)>) {
        crate::tui_dex::utils::logger::log_info(&format!(
            "Updating available pools for admin screen: {} pools found",
            pools.len()
        ));

        self.available_pools = pools.clone();

        // Update all pool dropdowns while preserving focus state
        {
            let was_active = self.pool_management.pool_selection_dropdown.is_active;
            let was_editing = self.pool_management.pool_selection_dropdown.is_editing;

            let options: Vec<SimpleListOption> = pools
                .iter()
                .map(|(pool_id, display_name)| {
                    SimpleListOption::new(display_name.clone(), pool_id.clone())
                })
                .collect();

            self.pool_management.pool_selection_dropdown.options = options;
            self.pool_management.pool_selection_dropdown.is_active = was_active;
            self.pool_management.pool_selection_dropdown.is_editing = was_editing;

            // Reset selection
            self.pool_management.pool_selection_dropdown.selected_index = None;
            if !pools.is_empty() {
                self.pool_management
                    .pool_selection_dropdown
                    .list_state
                    .select(Some(0));
            } else {
                self.pool_management
                    .pool_selection_dropdown
                    .list_state
                    .select(None);
            }
        }

        {
            let was_active = self.feature_control.pool_selection_dropdown.is_active;
            let was_editing = self.feature_control.pool_selection_dropdown.is_editing;

            let options: Vec<SimpleListOption> = pools
                .iter()
                .map(|(pool_id, display_name)| {
                    SimpleListOption::new(display_name.clone(), pool_id.clone())
                })
                .collect();

            self.feature_control.pool_selection_dropdown.options = options;
            self.feature_control.pool_selection_dropdown.is_active = was_active;
            self.feature_control.pool_selection_dropdown.is_editing = was_editing;

            // Reset selection
            self.feature_control.pool_selection_dropdown.selected_index = None;
            if !pools.is_empty() {
                self.feature_control
                    .pool_selection_dropdown
                    .list_state
                    .select(Some(0));
            } else {
                self.feature_control
                    .pool_selection_dropdown
                    .list_state
                    .select(None);
            }
        }
    }

    /// Switch admin mode/tab
    pub fn set_mode(&mut self, mode: AdminMode) {
        if self.mode != mode {
            self.mode = mode;
            self.clear_focus();

            // Reset focus to appropriate first input for the new mode
            self.input_focus = match mode {
                AdminMode::PoolManagement => AdminInputFocus::PoolSelection,
                AdminMode::PoolCreation => AdminInputFocus::FirstAssetDenom,
                AdminMode::FeatureControls => AdminInputFocus::TargetPoolId,
            };

            self.apply_focus();
            crate::tui_dex::utils::logger::log_info(&format!("Admin mode switched to {:?}", mode));
        }
    }

    /// Move focus to next input
    pub fn next_focus(&mut self) {
        self.input_focus = match self.mode {
            AdminMode::PoolManagement => match self.input_focus {
                AdminInputFocus::PoolSelection => AdminInputFocus::FeatureToggles,
                AdminInputFocus::FeatureToggles => AdminInputFocus::ManagementExecute,
                AdminInputFocus::ManagementExecute => AdminInputFocus::PoolSelection,
                _ => AdminInputFocus::PoolSelection,
            },
            AdminMode::PoolCreation => match self.input_focus {
                AdminInputFocus::FirstAssetDenom => AdminInputFocus::SecondAssetDenom,
                AdminInputFocus::SecondAssetDenom => AdminInputFocus::SwapFee,
                AdminInputFocus::SwapFee => AdminInputFocus::ProtocolFee,
                AdminInputFocus::ProtocolFee => AdminInputFocus::BurnFee,
                AdminInputFocus::BurnFee => AdminInputFocus::PoolType,
                AdminInputFocus::PoolType => AdminInputFocus::CreationExecute,
                AdminInputFocus::CreationExecute => AdminInputFocus::FirstAssetDenom,
                _ => AdminInputFocus::FirstAssetDenom,
            },
            AdminMode::FeatureControls => match self.input_focus {
                AdminInputFocus::TargetPoolId => AdminInputFocus::FeatureControls,
                AdminInputFocus::FeatureControls => AdminInputFocus::ControlsExecute,
                AdminInputFocus::ControlsExecute => AdminInputFocus::TargetPoolId,
                _ => AdminInputFocus::TargetPoolId,
            },
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Move focus to previous input
    pub fn previous_focus(&mut self) {
        self.input_focus = match self.mode {
            AdminMode::PoolManagement => match self.input_focus {
                AdminInputFocus::PoolSelection => AdminInputFocus::ManagementExecute,
                AdminInputFocus::FeatureToggles => AdminInputFocus::PoolSelection,
                AdminInputFocus::ManagementExecute => AdminInputFocus::FeatureToggles,
                _ => AdminInputFocus::ManagementExecute,
            },
            AdminMode::PoolCreation => match self.input_focus {
                AdminInputFocus::FirstAssetDenom => AdminInputFocus::CreationExecute,
                AdminInputFocus::SecondAssetDenom => AdminInputFocus::FirstAssetDenom,
                AdminInputFocus::SwapFee => AdminInputFocus::SecondAssetDenom,
                AdminInputFocus::ProtocolFee => AdminInputFocus::SwapFee,
                AdminInputFocus::BurnFee => AdminInputFocus::ProtocolFee,
                AdminInputFocus::PoolType => AdminInputFocus::BurnFee,
                AdminInputFocus::CreationExecute => AdminInputFocus::PoolType,
                _ => AdminInputFocus::CreationExecute,
            },
            AdminMode::FeatureControls => match self.input_focus {
                AdminInputFocus::TargetPoolId => AdminInputFocus::ControlsExecute,
                AdminInputFocus::FeatureControls => AdminInputFocus::TargetPoolId,
                AdminInputFocus::ControlsExecute => AdminInputFocus::FeatureControls,
                _ => AdminInputFocus::ControlsExecute,
            },
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Clear focus from all inputs
    fn clear_focus(&mut self) {
        // Pool management
        self.pool_management
            .pool_selection_dropdown
            .set_active(false);

        // Pool creation
        self.pool_creation.first_asset_input.set_focused(false);
        self.pool_creation.second_asset_input.set_focused(false);
        self.pool_creation.swap_fee_input.set_focused(false);
        self.pool_creation.protocol_fee_input.set_focused(false);
        self.pool_creation.burn_fee_input.set_focused(false);
        self.pool_creation.pool_type_dropdown.set_active(false);

        // Feature controls
        self.feature_control
            .pool_selection_dropdown
            .set_active(false);
    }

    /// Public wrapper to clear all focus states
    pub fn reset_focus(&mut self) {
        self.clear_focus();
    }

    /// Set focus on current input
    fn set_focus(&mut self) {
        match self.input_focus {
            // Pool Management
            AdminInputFocus::PoolSelection => {
                self.pool_management
                    .pool_selection_dropdown
                    .set_active(true);
            }
            AdminInputFocus::FeatureToggles => {} // Special handling for feature toggles
            AdminInputFocus::ManagementExecute => {} // Button focus handled separately

            // Pool Creation
            AdminInputFocus::FirstAssetDenom => {
                self.pool_creation.first_asset_input.set_focused(true);
            }
            AdminInputFocus::SecondAssetDenom => {
                self.pool_creation.second_asset_input.set_focused(true);
            }
            AdminInputFocus::SwapFee => {
                self.pool_creation.swap_fee_input.set_focused(true);
            }
            AdminInputFocus::ProtocolFee => {
                self.pool_creation.protocol_fee_input.set_focused(true);
            }
            AdminInputFocus::BurnFee => {
                self.pool_creation.burn_fee_input.set_focused(true);
            }
            AdminInputFocus::PoolType => {
                self.pool_creation.pool_type_dropdown.set_active(true);
                // Ensure list state is properly initialized for navigation
                if self
                    .pool_creation
                    .pool_type_dropdown
                    .list_state
                    .selected()
                    .is_none()
                    && !self.pool_creation.pool_type_dropdown.options.is_empty()
                {
                    self.pool_creation
                        .pool_type_dropdown
                        .list_state
                        .select(Some(0));
                }
            }
            AdminInputFocus::CreationExecute => {} // Button focus handled separately

            // Feature Controls
            AdminInputFocus::TargetPoolId => {
                self.feature_control
                    .pool_selection_dropdown
                    .set_active(true);
            }
            AdminInputFocus::FeatureControls => {} // Special handling for feature controls
            AdminInputFocus::ControlsExecute => {} // Button focus handled separately
        }
    }

    /// Public wrapper to apply focus
    pub fn apply_focus(&mut self) {
        self.clear_focus();
        self.set_focus();
    }

    /// Mark input change
    pub fn mark_input_change(&mut self) {
        self.last_input_change = Some(std::time::Instant::now());
    }

    /// Check if any list is currently in editing mode
    pub fn is_any_list_editing(&self) -> bool {
        self.pool_management.pool_selection_dropdown.is_editing
            || self.pool_creation.pool_type_dropdown.is_editing
            || self.feature_control.pool_selection_dropdown.is_editing
    }

    /// Perform validation checks and return detailed results
    fn validate_internal(&mut self) -> ValidationResult {
        match self.mode {
            AdminMode::PoolManagement => {
                let pool_selected = self
                    .pool_management
                    .pool_selection_dropdown
                    .get_selected_value()
                    .is_some();

                ValidationResult {
                    is_valid: pool_selected,
                    errors: if pool_selected {
                        Vec::new()
                    } else {
                        vec!["Please select a pool to manage".to_string()]
                    },
                }
            }
            AdminMode::PoolCreation => {
                let first_valid = self.pool_creation.first_asset_input.validate();
                let second_valid = self.pool_creation.second_asset_input.validate();
                let swap_fee_valid = self.pool_creation.swap_fee_input.validate();
                let protocol_fee_valid = self.pool_creation.protocol_fee_input.validate();
                let burn_fee_valid = self.pool_creation.burn_fee_input.validate();
                let pool_type_valid = self
                    .pool_creation
                    .pool_type_dropdown
                    .get_selected_value()
                    .is_some();

                let mut errors = Vec::new();
                if !first_valid {
                    errors.push("Please enter a valid first asset denomination".to_string());
                }
                if !second_valid {
                    errors.push("Please enter a valid second asset denomination".to_string());
                }
                if !swap_fee_valid {
                    errors.push("Please enter a valid swap fee (0-20%)".to_string());
                }
                if !protocol_fee_valid {
                    errors.push("Please enter a valid protocol fee (0-20%)".to_string());
                }
                if !burn_fee_valid {
                    errors.push("Please enter a valid burn fee (0-20%)".to_string());
                }
                if !pool_type_valid {
                    errors.push("Please select a pool type".to_string());
                }

                ValidationResult {
                    is_valid: first_valid
                        && second_valid
                        && swap_fee_valid
                        && protocol_fee_valid
                        && burn_fee_valid
                        && pool_type_valid,
                    errors,
                }
            }
            AdminMode::FeatureControls => {
                let pool_selected = self
                    .feature_control
                    .pool_selection_dropdown
                    .get_selected_value()
                    .is_some();

                ValidationResult {
                    is_valid: pool_selected,
                    errors: if pool_selected {
                        Vec::new()
                    } else {
                        vec!["Please select a pool to control".to_string()]
                    },
                }
            }
        }
    }

    /// Validate current form inputs
    pub fn validate(&mut self) -> bool {
        self.validate_internal().is_valid
    }

    /// Get detailed validation errors
    pub fn get_validation_errors(&mut self) -> Vec<String> {
        self.validate_internal().errors
    }

    /// Handle keyboard input using direct key events (like swap/liquidity screens)
    pub fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        navigation_mode: crate::tui_dex::app::NavigationMode,
    ) -> bool {
        use crossterm::event::KeyCode;

        // Handle admin internal tab switching (1-3) only when NOT in a text input field
        // This prevents tab switching when typing/pasting text that contains numbers
        let is_text_input_focused = matches!(
            self.input_focus,
            AdminInputFocus::FirstAssetDenom
                | AdminInputFocus::SecondAssetDenom
                | AdminInputFocus::SwapFee
                | AdminInputFocus::ProtocolFee
                | AdminInputFocus::BurnFee
        );

        if !is_text_input_focused {
            match key.code {
                KeyCode::Char('1') => {
                    self.set_mode(AdminMode::PoolManagement);
                    return true;
                }
                KeyCode::Char('2') => {
                    self.set_mode(AdminMode::PoolCreation);
                    return true;
                }
                KeyCode::Char('3') => {
                    self.set_mode(AdminMode::FeatureControls);
                    return true;
                }
                _ => {}
            }
        }

        // Only handle other events when in WithinScreen mode
        if navigation_mode != crate::tui_dex::app::NavigationMode::WithinScreen {
            return false;
        }

        // Handle ESC key to return to screen-level navigation
        if matches!(key.code, KeyCode::Esc) {
            return true; // Let the main app handle switching navigation modes
        }

        // Handle Tab navigation between fields
        if matches!(key.code, KeyCode::Tab) {
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT)
            {
                self.previous_focus();
            } else {
                self.next_focus();
            }
            return true;
        }

        // Log significant key events for admin operations
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
            && matches!(
                self.input_focus,
                AdminInputFocus::ManagementExecute
                    | AdminInputFocus::CreationExecute
                    | AdminInputFocus::ControlsExecute
            )
        {
            crate::tui_dex::utils::logger::log_info("=== ADMIN EXECUTE KEY PRESSED ===");
            crate::tui_dex::utils::logger::log_debug(&format!("Key event: {:?}", key));
            crate::tui_dex::utils::logger::log_debug(&format!(
                "Current focus: {:?}",
                self.input_focus
            ));
        }

        // Handle regular input focus
        match self.input_focus {
            // Pool Management
            AdminInputFocus::PoolSelection => {
                let list_event = self
                    .pool_management
                    .pool_selection_dropdown
                    .handle_key_event(key);

                if list_event == ListEvent::SelectionMade {
                    self.mark_input_change();
                    // Load pool features when a pool is selected
                    if let Some(pool_id) = self
                        .pool_management
                        .pool_selection_dropdown
                        .get_selected_value()
                    {
                        crate::tui_dex::utils::logger::log_info(&format!(
                            "Pool selected for management: {}",
                            pool_id
                        ));
                        // TODO: Load actual pool features from blockchain
                        self.pool_management.selected_pool_features = Some((true, true, true));
                    }
                }

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }

            AdminInputFocus::FeatureToggles => {
                // Handle feature toggle keys (W, D, S)
                match key.code {
                    KeyCode::Char('w') | KeyCode::Char('W') => {
                        // Toggle withdrawals
                        crate::tui_dex::utils::logger::log_info("Toggling withdrawal feature");
                        self.mark_input_change();
                        true
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        // Toggle deposits
                        crate::tui_dex::utils::logger::log_info("Toggling deposit feature");
                        self.mark_input_change();
                        true
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        // Toggle swaps
                        crate::tui_dex::utils::logger::log_info("Toggling swap feature");
                        self.mark_input_change();
                        true
                    }
                    _ => false,
                }
            }

            AdminInputFocus::ManagementExecute => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            self.mark_input_change();
                            crate::tui_dex::utils::logger::log_info(
                                "Pool management execute button pressed",
                            );
                            return true;
                        }
                    }
                    _ => {}
                }
                false
            }

            // Pool Creation
            AdminInputFocus::FirstAssetDenom => {
                let input_request = self.map_key_to_input_request(key);
                if let Some(request) = input_request {
                    if self
                        .pool_creation
                        .first_asset_input
                        .handle_input(request)
                        .is_some()
                    {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }

            AdminInputFocus::SecondAssetDenom => {
                let input_request = self.map_key_to_input_request(key);
                if let Some(request) = input_request {
                    if self
                        .pool_creation
                        .second_asset_input
                        .handle_input(request)
                        .is_some()
                    {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }

            AdminInputFocus::SwapFee => {
                let input_request = self.map_key_to_input_request(key);
                if let Some(request) = input_request {
                    if self
                        .pool_creation
                        .swap_fee_input
                        .handle_input(request)
                        .is_some()
                    {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }

            AdminInputFocus::ProtocolFee => {
                let input_request = self.map_key_to_input_request(key);
                if let Some(request) = input_request {
                    if self
                        .pool_creation
                        .protocol_fee_input
                        .handle_input(request)
                        .is_some()
                    {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }

            AdminInputFocus::BurnFee => {
                let input_request = self.map_key_to_input_request(key);
                if let Some(request) = input_request {
                    if self
                        .pool_creation
                        .burn_fee_input
                        .handle_input(request)
                        .is_some()
                    {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }

            AdminInputFocus::PoolType => {
                let list_event = self.pool_creation.pool_type_dropdown.handle_key_event(key);

                if list_event == ListEvent::SelectionMade {
                    self.mark_input_change();
                    // When a selection is made, sync the selected_index with the highlighted item
                    if let Some(highlighted_idx) =
                        self.pool_creation.pool_type_dropdown.list_state.selected()
                    {
                        self.pool_creation.pool_type_dropdown.selected_index =
                            Some(highlighted_idx);
                    }
                }

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }

            AdminInputFocus::CreationExecute => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            self.mark_input_change();
                            crate::tui_dex::utils::logger::log_info(
                                "Pool creation execute button pressed - validation passed",
                            );
                            // Trigger pool creation confirmation
                            return true; // Event will be handled by app to show confirmation
                        } else {
                            crate::tui_dex::utils::logger::log_warning(
                                "Pool creation validation failed - please check all fields",
                            );
                        }
                    }
                    _ => {}
                }
                false
            }

            // Feature Controls
            AdminInputFocus::TargetPoolId => {
                let list_event = self
                    .feature_control
                    .pool_selection_dropdown
                    .handle_key_event(key);

                if list_event == ListEvent::SelectionMade {
                    self.mark_input_change();
                }

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }

            AdminInputFocus::FeatureControls => {
                // Handle bulk feature control keys
                match key.code {
                    KeyCode::Char('1') => {
                        crate::tui_dex::utils::logger::log_info("Enable all features requested");
                        self.mark_input_change();
                        true
                    }
                    KeyCode::Char('2') => {
                        crate::tui_dex::utils::logger::log_info("Disable all features requested");
                        self.mark_input_change();
                        true
                    }
                    _ => false,
                }
            }

            AdminInputFocus::ControlsExecute => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            self.mark_input_change();
                            crate::tui_dex::utils::logger::log_info(
                                "Feature controls execute button pressed",
                            );
                            return true;
                        }
                    }
                    _ => {}
                }
                false
            }
        }
    }

    /// Map key event to input request (helper method)
    fn map_key_to_input_request(&self, key: crossterm::event::KeyEvent) -> Option<InputRequest> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
            KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
            KeyCode::Delete => Some(InputRequest::DeleteNextChar),
            KeyCode::Left => Some(InputRequest::GoToPrevChar),
            KeyCode::Right => Some(InputRequest::GoToNextChar),
            KeyCode::Home => Some(InputRequest::GoToStart),
            KeyCode::End => Some(InputRequest::GoToEnd),
            _ => None,
        }
    }

    /// Handle keyboard input (legacy method - kept for compatibility)
    pub fn handle_input(&mut self, input: InputRequest) -> bool {
        // This method is kept for backward compatibility
        false
    }
}

/// Pool creation details for confirmation
#[derive(Debug, Clone)]
pub struct PoolCreationDetails {
    pub first_asset: String,
    pub second_asset: String,
    pub swap_fee: String,
    pub protocol_fee: String,
    pub burn_fee: String,
    pub pool_type: String,
}

/// Feature management details for confirmation
#[derive(Debug, Clone)]
pub struct FeatureManagementDetails {
    pub pool_id: String,
    pub withdrawals_enabled: bool,
    pub deposits_enabled: bool,
    pub swaps_enabled: bool,
}

// Global admin screen state - like swap and liquidity screens
static mut ADMIN_SCREEN_STATE: Option<AdminScreenState> = None;

/// Get or initialize the admin screen state
pub(crate) fn get_admin_screen_state() -> &'static mut AdminScreenState {
    unsafe {
        if ADMIN_SCREEN_STATE.is_none() {
            ADMIN_SCREEN_STATE = Some(AdminScreenState::default());
        }
        ADMIN_SCREEN_STATE.as_mut().unwrap()
    }
}

/// Render the complete admin screen (consistent with swap/liquidity screens)
pub fn render_admin(f: &mut Frame, app: &App) {
    let size = f.area();

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

    // Render header and navigation
    render_header(f, &app.state, chunks[0]);
    render_navigation(f, &app.state, chunks[1]);

    // Render admin content
    render_admin_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);

    // Render validation overlay if needed
    if app.state.current_screen == crate::tui_dex::app::Screen::Admin {
        render_validation_overlay(f, size, app);
    }
}

/// Render the main admin content area (consistent with swap/liquidity screens)
fn render_admin_content(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    // Create vertical layout: tabs + content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Render admin tabs using Tabs widget (like liquidity screen)
    let tabs = vec!["Pool Management", "Pool Creation", "Feature Controls"];
    let tab_index = match admin_state.mode {
        AdminMode::PoolManagement => 0,
        AdminMode::PoolCreation => 1,
        AdminMode::FeatureControls => 2,
    };

    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Admin Panel"),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(tab_index);

    f.render_widget(tabs_widget, main_chunks[0]);

    // Render content based on current mode
    match admin_state.mode {
        AdminMode::PoolManagement => render_pool_management_panel(f, main_chunks[1], app),
        AdminMode::PoolCreation => render_pool_creation_panel(f, main_chunks[1], app),
        AdminMode::FeatureControls => render_feature_controls_panel(f, main_chunks[1], app),
    }
}

/// Render pool creation panel (consistent with swap/liquidity form patterns)
fn render_pool_creation_panel(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left side: Creation form
    render_pool_creation_form(f, chunks[0], app);

    // Right side: Preview and validation
    render_pool_creation_preview(f, chunks[1], app);
}

/// Render pool creation form (like swap/liquidity input forms)
fn render_pool_creation_form(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5), // First asset
            Constraint::Length(5), // Second asset
            Constraint::Length(5), // Swap fee
            Constraint::Length(5), // Protocol fee
            Constraint::Length(5), // Burn fee
            Constraint::Length(8), // Pool type dropdown
            Constraint::Length(5), // Execute button
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Create New Pool");
    f.render_widget(block, area);

    // Render input fields
    admin_state
        .pool_creation
        .first_asset_input
        .render(f, chunks[0]);
    admin_state
        .pool_creation
        .second_asset_input
        .render(f, chunks[1]);
    admin_state
        .pool_creation
        .swap_fee_input
        .render(f, chunks[2]);
    admin_state
        .pool_creation
        .protocol_fee_input
        .render(f, chunks[3]);
    admin_state
        .pool_creation
        .burn_fee_input
        .render(f, chunks[4]);

    // Pool type dropdown
    let admin_state_mut = get_admin_screen_state();
    admin_state_mut
        .pool_creation
        .pool_type_dropdown
        .render(f, chunks[5]);

    // Execute button
    render_creation_execute_button(f, chunks[6], app);
}

/// Render pool creation preview
fn render_pool_creation_preview(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Pool Preview");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let first_asset = admin_state.pool_creation.first_asset_input.value();
    let second_asset = admin_state.pool_creation.second_asset_input.value();
    let swap_fee = admin_state.pool_creation.swap_fee_input.value();
    let protocol_fee = admin_state.pool_creation.protocol_fee_input.value();
    let burn_fee = admin_state.pool_creation.burn_fee_input.value();
    let pool_type = admin_state
        .pool_creation
        .pool_type_dropdown
        .get_selected_label()
        .unwrap_or("Not selected");

    let preview_content = if first_asset.is_empty() || second_asset.is_empty() {
        "Enter asset denominations to see preview".to_string()
    } else {
        format!(
            "Pool Preview:\n\n• Asset Pair: {} / {}\n• Pool Type: {}\n\nFee Structure:\n• Swap Fee: {}%\n• Protocol Fee: {}%\n• Burn Fee: {}%\n• Total Fee: {:.2}%\n\nThis pool will be created on the Mantra DEX\nwith the specified configuration.",
            first_asset,
            second_asset,
            pool_type,
            swap_fee,
            protocol_fee,
            burn_fee,
            swap_fee.parse::<f64>().unwrap_or(0.0) +
            protocol_fee.parse::<f64>().unwrap_or(0.0) +
            burn_fee.parse::<f64>().unwrap_or(0.0)
        )
    };

    let paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render creation execute button (like swap/liquidity execute buttons)
fn render_creation_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();
    let is_focused = matches!(admin_state.input_focus, AdminInputFocus::CreationExecute);
    let is_valid = admin_state.clone().validate();

    let is_loading = matches!(app.state.loading_state, LoadingState::Loading { .. });
    let loading_message = if let LoadingState::Loading {
        message, progress, ..
    } = &app.state.loading_state
    {
        if let Some(p) = progress {
            format!("{} ({}%)", message, *p as u16)
        } else {
            message.clone()
        }
    } else {
        String::new()
    };

    let (button_style, button_text, border_style) = if is_loading {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            if loading_message.is_empty() {
                "Creating Pool..."
            } else {
                &loading_message
            },
            Style::default().fg(Color::Yellow),
        )
    } else if !is_valid {
        (
            Style::default().fg(Color::DarkGray),
            "Invalid Input",
            Style::default().fg(Color::Gray),
        )
    } else if is_focused {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "► Create Pool ◄",
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "Create Pool",
            Style::default().fg(Color::Green),
        )
    };

    let button_content = if is_loading {
        let dots = match (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 500)
            % 4
        {
            0 => "",
            1 => ".",
            2 => "..",
            _ => "...",
        };
        format!("{}{}", button_text, dots)
    } else {
        button_text.to_string()
    };

    let button = Paragraph::new(button_content)
        .style(button_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(if is_loading { "Processing" } else { "Action" }),
        );

    f.render_widget(button, area);
}

/// Render pool management panel (consistent with swap/liquidity patterns)
fn render_pool_management_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left: Pool list and selection
    render_pool_management_form(f, chunks[0], app);

    // Right: Pool details and feature status
    render_pool_management_details(f, chunks[1], app);
}

/// Render pool management form
fn render_pool_management_form(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(8), // Pool selection dropdown
            Constraint::Length(8), // Feature toggles
            Constraint::Length(5), // Execute button
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Manage Pool Features");
    f.render_widget(block, area);

    // Pool selection dropdown
    let admin_state_mut = get_admin_screen_state();
    admin_state_mut
        .pool_management
        .pool_selection_dropdown
        .render(f, chunks[0]);

    // Feature toggles
    render_pool_feature_toggles(f, chunks[1], app);

    // Execute button
    render_management_execute_button(f, chunks[2], app);
}

/// Render pool feature toggles
fn render_pool_feature_toggles(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();
    let is_focused = matches!(admin_state.input_focus, AdminInputFocus::FeatureToggles);

    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title("Feature Controls")
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let features = admin_state
        .pool_management
        .selected_pool_features
        .unwrap_or((true, true, true));

    let content = vec![
        Line::from(vec![
            Span::styled("• [W] Withdrawals: ", Style::default().fg(Color::White)),
            Span::styled(
                if features.0 { "ENABLED" } else { "DISABLED" },
                if features.0 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("• [D] Deposits: ", Style::default().fg(Color::White)),
            Span::styled(
                if features.1 { "ENABLED" } else { "DISABLED" },
                if features.1 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("• [S] Swaps: ", Style::default().fg(Color::White)),
            Span::styled(
                if features.2 { "ENABLED" } else { "DISABLED" },
                if features.2 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press key to toggle feature",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content)).wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render management execute button
fn render_management_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();
    let is_focused = matches!(admin_state.input_focus, AdminInputFocus::ManagementExecute);
    let is_valid = admin_state.clone().validate();

    let is_loading = matches!(app.state.loading_state, LoadingState::Loading { .. });

    let (button_style, button_text, border_style) = if is_loading {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            "Updating Features...",
            Style::default().fg(Color::Yellow),
        )
    } else if !is_valid {
        (
            Style::default().fg(Color::DarkGray),
            "Select Pool",
            Style::default().fg(Color::Gray),
        )
    } else if is_focused {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            "► Apply Changes ◄",
            Style::default().fg(Color::Blue),
        )
    } else {
        (
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            "Apply Changes",
            Style::default().fg(Color::Blue),
        )
    };

    let button = Paragraph::new(button_text)
        .style(button_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Action"),
        );

    f.render_widget(button, area);
}

/// Render pool management details
fn render_pool_management_details(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Pool Details");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = if let Some(pool_id) = admin_state
        .pool_management
        .pool_selection_dropdown
        .get_selected_value()
    {
        let features = admin_state
            .pool_management
            .selected_pool_features
            .unwrap_or((true, true, true));
        format!(
            "Selected Pool: {}\n\nCurrent Feature Status:\n• Withdrawals: {}\n• Deposits: {}\n• Swaps: {}\n\nPool Management:\n• Real-time feature control\n• Immediate blockchain updates\n• Admin privileges required\n\nChanges will be applied to the\nMantra DEX smart contract.",
            pool_id,
            if features.0 { "Enabled" } else { "Disabled" },
            if features.1 { "Enabled" } else { "Disabled" },
            if features.2 { "Enabled" } else { "Disabled" }
        )
    } else {
        "Select a pool to view details\n\nPool Management Features:\n• Enable/disable withdrawals\n• Enable/disable deposits\n• Enable/disable swaps\n• Real-time status updates\n\nAll changes are applied directly\nto the blockchain.".to_string()
    };

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render feature controls panel
fn render_feature_controls_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left: Bulk feature controls
    render_feature_controls_form(f, chunks[0], app);

    // Right: Bulk operations help
    render_feature_controls_details(f, chunks[1], app);
}

/// Render feature controls form
fn render_feature_controls_form(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(8), // Pool selection
            Constraint::Length(8), // Bulk controls
            Constraint::Length(5), // Execute button
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Bulk Feature Controls");
    f.render_widget(block, area);

    // Pool selection
    let admin_state_mut = get_admin_screen_state();
    admin_state_mut
        .feature_control
        .pool_selection_dropdown
        .render(f, chunks[0]);

    // Bulk controls
    render_bulk_feature_controls(f, chunks[1], app);

    // Execute button
    render_controls_execute_button(f, chunks[2], app);
}

/// Render bulk feature controls
fn render_bulk_feature_controls(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();
    let is_focused = matches!(admin_state.input_focus, AdminInputFocus::FeatureControls);

    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title("Bulk Operations")
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = vec![
        Line::from(vec![Span::styled(
            "Bulk Feature Operations:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• [1] ", Style::default().fg(Color::Green)),
            Span::styled("Enable All Features", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [2] ", Style::default().fg(Color::Red)),
            Span::styled("Disable All Features", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "⚠️ Warning: Changes are immediate!",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content)).wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render controls execute button
fn render_controls_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let admin_state = get_admin_screen_state();
    let is_focused = matches!(admin_state.input_focus, AdminInputFocus::ControlsExecute);
    let is_valid = admin_state.clone().validate();

    let is_loading = matches!(app.state.loading_state, LoadingState::Loading { .. });

    let (button_style, button_text, border_style) = if is_loading {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            "Applying Changes...",
            Style::default().fg(Color::Yellow),
        )
    } else if !is_valid {
        (
            Style::default().fg(Color::DarkGray),
            "Select Pool",
            Style::default().fg(Color::Gray),
        )
    } else if is_focused {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
            "► Execute ◄",
            Style::default().fg(Color::Red),
        )
    } else {
        (
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            "Execute",
            Style::default().fg(Color::Red),
        )
    };

    let button = Paragraph::new(button_text)
        .style(button_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Action"),
        );

    f.render_widget(button, area);
}

/// Render feature controls details
fn render_feature_controls_details(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Bulk Operations Help");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = "Bulk Feature Controls:\n\n• Select target pool\n• Choose bulk operation\n• Apply to all features at once\n\nAvailable Operations:\n• Enable All: Activates withdrawals, deposits, and swaps\n• Disable All: Deactivates all pool operations\n\nSafety Notes:\n• Changes are immediate\n• Cannot be undone easily\n• Affects all pool users\n• Requires admin privileges\n\nUse with caution in production!";

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render pool creation progress indicator
fn render_creation_progress(f: &mut Frame, area: Rect) {
    let current_step = 1; // This would come from state
    let total_steps = 5;
    let progress = (current_step as f64 / total_steps as f64) * 100.0;

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(format!(
                    "Pool Creation Progress (Step {} of {})",
                    current_step, total_steps
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(progress as u16);

    f.render_widget(gauge, area);
}

/// Render current creation step content
fn render_creation_step_content(f: &mut Frame, area: Rect) {
    // For now, render asset selection step
    render_asset_selection_step(f, area);
}

/// Render asset selection step
fn render_asset_selection_step(f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Asset input
    render_asset_input(f, chunks[0]);

    // Right: Asset list
    render_asset_list(f, chunks[1]);
}

/// Render asset input form
fn render_asset_input(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Add Assets")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Asset Denom:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter asset denomination]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Decimals:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter decimal places]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Enter to add asset",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render current asset list
fn render_asset_list(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Pool Assets")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Assets (min 2 required):",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::White)),
            Span::styled("No assets added yet", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pool creation requires:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from("  - At least 2 assets"),
        Line::from("  - Valid decimal configuration"),
        Line::from("  - Fee structure (max 20% total)"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render creation navigation help
fn render_creation_navigation_help(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Navigation")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![
            Span::styled("• [Tab] ", Style::default().fg(Color::Green)),
            Span::styled("Next field  ", Style::default().fg(Color::White)),
            Span::styled("• [Shift+Tab] ", Style::default().fg(Color::Green)),
            Span::styled("Previous field", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [Enter] ", Style::default().fg(Color::Cyan)),
            Span::styled("Next step  ", Style::default().fg(Color::White)),
            Span::styled("• [Esc] ", Style::default().fg(Color::Red)),
            Span::styled("Cancel creation", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render feature controls interface
fn render_feature_controls(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Global controls
            Constraint::Min(0),    // Pool-specific controls
        ])
        .split(area);

    // Global feature controls
    render_global_feature_controls(f, chunks[0]);

    // Pool-specific controls
    render_pool_specific_controls(f, chunks[1], app);
}

/// Render global feature controls
fn render_global_feature_controls(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Global Feature Updates")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "⚠️  Global Operations:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Note: v3.0.0 requires per-pool targeting"),
        Line::from("Global operations apply to specified pools only"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Use Pool Management tab for individual pool control",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render pool-specific controls
fn render_pool_specific_controls(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Pool selection and features
    render_feature_pool_selection(f, chunks[0]);

    // Right: Bulk operations
    render_bulk_operations(f, chunks[1], app);
}

/// Render feature pool selection
fn render_feature_pool_selection(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Pool Feature Management")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Target Pool ID:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter pool ID to modify]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Available Operations:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("• Toggle individual features"),
        Line::from("• Enable/disable all operations"),
        Line::from("• View current feature status"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render bulk operations panel
fn render_bulk_operations(f: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .title("Bulk Operations")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Multi-Pool Operations:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• [1] ", Style::default().fg(Color::Green)),
            Span::styled(
                "Enable all features for pool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• [2] ", Style::default().fg(Color::Red)),
            Span::styled(
                "Disable all features for pool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Warning:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Changes are immediate!", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render validation error overlay (like swap/liquidity screens)
fn render_validation_overlay(f: &mut Frame, area: Rect, _app: &App) {
    let admin_state = get_admin_screen_state();

    // Only show validation errors when execute button is focused and validation fails
    if !matches!(
        admin_state.input_focus,
        AdminInputFocus::ManagementExecute
            | AdminInputFocus::CreationExecute
            | AdminInputFocus::ControlsExecute
    ) {
        return;
    }

    if admin_state.clone().validate() {
        return; // No errors to display
    }

    let errors = admin_state.clone().get_validation_errors();
    if errors.is_empty() {
        return;
    }

    // Create a small overlay at the bottom of the screen
    let overlay_height = (errors.len() + 2).min(6) as u16;
    let overlay_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - overlay_height - 4,
        width: area.width - 4,
        height: overlay_height,
    };

    // Clear the area
    f.render_widget(Clear, overlay_area);

    // Create error content
    let error_lines: Vec<Line> = errors
        .iter()
        .map(|error| {
            Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Red)),
                Span::styled(error, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let error_text = Text::from(error_lines);

    let error_block = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title("Validation Errors")
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap { trim: true });

    f.render_widget(error_block, overlay_area);
}

// Public API functions (like swap/liquidity screens)

/// Handle admin screen input (legacy compatibility)
pub fn handle_admin_screen_input(
    app_state: &mut crate::tui_dex::app::AppState,
    input: InputRequest,
) -> bool {
    app_state.admin_screen_state.handle_input(input)
}

/// Switch admin mode
pub fn switch_admin_mode(app_state: &mut crate::tui_dex::app::AppState, mode: AdminMode) {
    app_state.admin_screen_state.set_mode(mode);
}

/// Update available pools for admin operations
pub fn update_admin_pools(
    app_state: &mut crate::tui_dex::app::AppState,
    pools: Vec<(String, String)>,
) {
    app_state.admin_screen_state.update_available_pools(pools);
}

/// Initialize admin screen focus
pub fn initialize_admin_screen_focus(app_state: &mut crate::tui_dex::app::AppState) {
    app_state.admin_screen_state.input_focus = AdminInputFocus::PoolSelection;
    app_state.admin_screen_state.apply_focus();

    crate::tui_dex::utils::logger::log_info("Admin screen focus initialized");
}

/// Execute pool creation with confirmation
pub fn execute_pool_creation_with_confirmation(app_state: &mut crate::tui_dex::app::AppState) {
    let admin_state = &mut app_state.admin_screen_state;

    crate::tui_dex::utils::logger::log_info("=== POOL CREATION EXECUTION ATTEMPT ===");

    if !admin_state.validate() {
        let errors = admin_state.clone().get_validation_errors();
        crate::tui_dex::utils::logger::log_error("Pool creation validation failed:");
        for error in &errors {
            crate::tui_dex::utils::logger::log_error(&format!("  - {}", error));
        }
        return;
    }

    // Get current values from the form
    let first_asset = admin_state.pool_creation.first_asset_input.value();
    let second_asset = admin_state.pool_creation.second_asset_input.value();
    let swap_fee = admin_state.pool_creation.swap_fee_input.value();
    let protocol_fee = admin_state.pool_creation.protocol_fee_input.value();
    let burn_fee = admin_state.pool_creation.burn_fee_input.value();
    let pool_type = admin_state
        .pool_creation
        .pool_type_dropdown
        .get_selected_value()
        .unwrap_or_default();

    // Log pool creation parameters
    crate::tui_dex::utils::logger::log_info("Pool Creation parameters:");
    crate::tui_dex::utils::logger::log_info(&format!("  First Asset: {}", first_asset));
    crate::tui_dex::utils::logger::log_info(&format!("  Second Asset: {}", second_asset));
    crate::tui_dex::utils::logger::log_info(&format!("  Swap Fee: {}%", swap_fee));
    crate::tui_dex::utils::logger::log_info(&format!("  Protocol Fee: {}%", protocol_fee));
    crate::tui_dex::utils::logger::log_info(&format!("  Burn Fee: {}%", burn_fee));
    crate::tui_dex::utils::logger::log_info(&format!("  Pool Type: {}", pool_type));

    let pool_details = PoolCreationDetails {
        first_asset: first_asset.to_string(),
        second_asset: second_asset.to_string(),
        swap_fee: swap_fee.to_string(),
        protocol_fee: protocol_fee.to_string(),
        burn_fee: burn_fee.to_string(),
        pool_type: pool_type.to_string(),
    };

    let confirmation_message = format!(
        "Confirm Pool Creation:\n\n• Asset Pair: {} / {}\n• Pool Type: {}\n• Swap Fee: {}%\n• Protocol Fee: {}%\n• Burn Fee: {}%\n• Total Fee: {:.2}%\n\nThis will create a new pool on the Mantra DEX.\nProceed with transaction?",
        pool_details.first_asset,
        pool_details.second_asset,
        pool_details.pool_type,
        pool_details.swap_fee,
        pool_details.protocol_fee,
        pool_details.burn_fee,
        pool_details.swap_fee.parse::<f64>().unwrap_or(0.0) +
        pool_details.protocol_fee.parse::<f64>().unwrap_or(0.0) +
        pool_details.burn_fee.parse::<f64>().unwrap_or(0.0)
    );

    crate::tui_dex::utils::logger::log_info("Pool creation confirmation modal prepared");
    crate::tui_dex::utils::logger::log_debug(&format!(
        "Confirmation message: {}",
        confirmation_message
    ));
    crate::tui_dex::utils::logger::log_info(&format!(
        "Pool creation confirmation ready: {}",
        confirmation_message
    ));
}

/// Execute pool feature management with confirmation
pub fn execute_pool_management_with_confirmation(app_state: &mut crate::tui_dex::app::AppState) {
    let admin_state = &mut app_state.admin_screen_state;

    crate::tui_dex::utils::logger::log_info("=== POOL MANAGEMENT EXECUTION ATTEMPT ===");

    if !admin_state.validate() {
        let errors = admin_state.clone().get_validation_errors();
        crate::tui_dex::utils::logger::log_error("Pool management validation failed:");
        for error in &errors {
            crate::tui_dex::utils::logger::log_error(&format!("  - {}", error));
        }
        return;
    }

    let pool_id = admin_state
        .pool_management
        .pool_selection_dropdown
        .get_selected_value()
        .unwrap_or_default();
    let features = admin_state
        .pool_management
        .selected_pool_features
        .unwrap_or((true, true, true));

    crate::tui_dex::utils::logger::log_info("Pool Management parameters:");
    crate::tui_dex::utils::logger::log_info(&format!("  Pool ID: {}", pool_id));
    crate::tui_dex::utils::logger::log_info(&format!("  Withdrawals: {}", features.0));
    crate::tui_dex::utils::logger::log_info(&format!("  Deposits: {}", features.1));
    crate::tui_dex::utils::logger::log_info(&format!("  Swaps: {}", features.2));

    let feature_details = FeatureManagementDetails {
        pool_id: pool_id.to_string(),
        withdrawals_enabled: features.0,
        deposits_enabled: features.1,
        swaps_enabled: features.2,
    };

    let confirmation_message = format!(
        "Confirm Feature Update:\n\n• Pool: {}\n• Withdrawals: {}\n• Deposits: {}\n• Swaps: {}\n\nThis will update pool features on the Mantra DEX.\nProceed with transaction?",
        feature_details.pool_id,
        if feature_details.withdrawals_enabled { "Enabled" } else { "Disabled" },
        if feature_details.deposits_enabled { "Enabled" } else { "Disabled" },
        if feature_details.swaps_enabled { "Enabled" } else { "Disabled" }
    );

    crate::tui_dex::utils::logger::log_info("Pool management confirmation modal prepared");
    crate::tui_dex::utils::logger::log_info(&format!(
        "Pool management confirmation ready: {}",
        confirmation_message
    ));
}

/// Handle pool creation confirmation response
pub fn handle_pool_creation_confirmation_response(
    app_state: &mut crate::tui_dex::app::AppState,
    confirmed: bool,
) -> Option<crate::tui_dex::events::Event> {
    crate::tui_dex::utils::logger::log_info(&format!(
        "=== POOL CREATION CONFIRMATION RESPONSE: {} ===",
        if confirmed { "CONFIRMED" } else { "CANCELLED" }
    ));

    if confirmed {
        let admin_state = &mut app_state.admin_screen_state;

        let first_asset = admin_state.pool_creation.first_asset_input.value();
        let second_asset = admin_state.pool_creation.second_asset_input.value();
        let swap_fee = admin_state.pool_creation.swap_fee_input.value();
        let protocol_fee = admin_state.pool_creation.protocol_fee_input.value();
        let burn_fee = admin_state.pool_creation.burn_fee_input.value();
        let pool_type = admin_state
            .pool_creation
            .pool_type_dropdown
            .get_selected_value()
            .unwrap_or_default();

        // Create pool creation event
        Some(crate::tui_dex::events::Event::CreatePool {
            asset_1: first_asset.to_string(),
            asset_2: second_asset.to_string(),
            swap_fee: swap_fee.to_string(),
            exit_fee: burn_fee.to_string(),
            pool_features: vec![
                format!("protocol_fee:{}", protocol_fee),
                format!("pool_type:{}", pool_type),
            ],
        })
    } else {
        None
    }
}

/// Handle pool management confirmation response
pub fn handle_pool_management_confirmation_response(
    app_state: &mut crate::tui_dex::app::AppState,
    confirmed: bool,
) -> Option<crate::tui_dex::events::Event> {
    crate::tui_dex::utils::logger::log_info(&format!(
        "=== POOL MANAGEMENT CONFIRMATION RESPONSE: {} ===",
        if confirmed { "CONFIRMED" } else { "CANCELLED" }
    ));

    if confirmed {
        let admin_state = &mut app_state.admin_screen_state;

        let pool_id = admin_state
            .pool_management
            .pool_selection_dropdown
            .get_selected_value()
            .unwrap_or_default();
        let features = admin_state
            .pool_management
            .selected_pool_features
            .unwrap_or((true, true, true));

        // Create pool management event
        Some(crate::tui_dex::events::Event::UpdatePoolFeatures {
            pool_id: pool_id.to_string(),
            features: vec![
                if features.0 {
                    "withdrawals".to_string()
                } else {
                    "".to_string()
                },
                if features.1 {
                    "deposits".to_string()
                } else {
                    "".to_string()
                },
                if features.2 {
                    "swaps".to_string()
                } else {
                    "".to_string()
                },
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
            enabled: features.0 || features.1 || features.2,
        })
    } else {
        None
    }
}

/// Reset admin forms
pub fn reset_admin_forms(app_state: &mut crate::tui_dex::app::AppState) {
    let admin_state = &mut app_state.admin_screen_state;

    // Preserve pool data before reset
    let available_pools = admin_state.available_pools.clone();

    // Reset form inputs
    admin_state.pool_creation.first_asset_input.clear();
    admin_state.pool_creation.second_asset_input.clear();
    admin_state.pool_creation.swap_fee_input.set_value("0.03");
    admin_state
        .pool_creation
        .protocol_fee_input
        .set_value("0.01");
    admin_state.pool_creation.burn_fee_input.set_value("0.0");

    // Reset pool management
    admin_state.pool_management.selected_pool_features = None;

    // Restore pool data
    admin_state.update_available_pools(available_pools);

    crate::tui_dex::utils::logger::log_info("Admin forms reset completed");
}
