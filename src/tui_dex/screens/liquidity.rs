//! Liquidity Screen Implementation
//!
//! This module provides the liquidity management interface for the MANTRA DEX SDK TUI,
//! allowing users to provide liquidity to pools, withdraw liquidity, view current positions,
//! and analyze position performance with PnL calculations.

use crate::tui_dex::{
    app::{App, LoadingState},
    components::{
        forms::{InputType, TextInput},
        header::render_header,
        modals::{render_modal, ModalState},
        navigation::render_navigation,
        simple_list::{ListEvent, SimpleList, SimpleListOption},
        status_bar::render_status_bar,
        // tables::format_large_number, // We'll define our own
    },
};
use cosmwasm_std::Uint128;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};
use tui_input::InputRequest;

/// Liquidity screen operation modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidityMode {
    Provide,
    Withdraw,
    Positions,
}

/// Input focus states for the liquidity screen (simplified like swap screen)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidityInputFocus {
    Pool,
    FirstAssetAmount,
    SecondAssetAmount,
    SlippageAmount,
    WithdrawAmount,
    Execute,
}

/// Current liquidity position information
#[derive(Debug, Clone)]
pub struct LiquidityPosition {
    pub pool_id: String,
    pub asset_pair: String,
    pub lp_token_amount: Uint128,
    pub estimated_value_usd: f64,
    pub initial_value_usd: f64,
    pub pnl_percentage: f64,
    pub pnl_usd: f64,
    pub share_percentage: f64,
    pub first_asset_amount: Uint128,
    pub second_asset_amount: Uint128,
    pub first_asset_denom: String,
    pub second_asset_denom: String,
}

/// Liquidity screen state (simplified like swap screen)
#[derive(Debug, Clone)]
pub struct LiquidityScreenState {
    /// Current operation mode
    pub mode: LiquidityMode,
    /// Current input focus
    pub input_focus: LiquidityInputFocus,
    /// Pool selection dropdown
    pub pool_dropdown: SimpleList,
    /// First asset amount input (for providing liquidity)
    pub first_asset_input: TextInput,
    /// Second asset amount input (for providing liquidity)
    pub second_asset_input: TextInput,
    /// Slippage tolerance input
    pub slippage_input: TextInput,
    /// LP token amount input (for withdrawing liquidity)
    pub withdraw_amount_input: TextInput,

    /// Available pools for liquidity operations
    pub available_pools: Vec<(String, String)>, // (pool_id, display_name)
    /// Current liquidity positions
    pub positions: Vec<LiquidityPosition>,
    /// Selected position index for details
    pub selected_position: Option<usize>,
    /// Expected LP tokens from providing liquidity
    pub expected_lp_tokens: Option<Uint128>,
    /// Expected assets from withdrawing liquidity
    pub expected_assets: Option<(Uint128, Uint128, String, String)>, // (amount1, amount2, denom1, denom2)
    /// Timer for auto-refresh
    pub last_input_change: Option<std::time::Instant>,
    /// Current pool reserves for proportional calculations (supports multi-asset pools)
    pub current_pool_reserves: Option<Vec<(Uint128, String)>>, // Vec of (reserve_amount, denom)
    /// Flag to prevent infinite loops during proportional calculation
    pub updating_proportional_amount: bool,
}

impl Default for LiquidityScreenState {
    fn default() -> Self {
        let pool_dropdown = SimpleList::new("Select Pool");

        let first_asset_input = TextInput::new("First Asset Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let second_asset_input = TextInput::new("Second Asset Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let slippage_input = TextInput::new("Slippage Tolerance (%)")
            .with_type(InputType::Amount)
            .with_value("1.0")
            .with_placeholder("1.0");

        let withdraw_amount_input = TextInput::new("LP Token Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let mut instance = Self {
            mode: LiquidityMode::Provide,
            input_focus: LiquidityInputFocus::Pool,
            pool_dropdown,
            first_asset_input,
            second_asset_input,
            slippage_input,
            withdraw_amount_input,
            available_pools: Vec::new(),
            positions: Vec::new(),
            selected_position: None,
            expected_lp_tokens: None,
            expected_assets: None,
            last_input_change: None,
            current_pool_reserves: None,
            updating_proportional_amount: false,
        };

        // Apply initial focus
        instance.apply_focus();
        instance
    }
}

impl LiquidityScreenState {
    /// Update available pools (simplified like swap screen)
    pub fn update_available_pools(&mut self, pools: Vec<(String, String)>) {
        crate::tui_dex::utils::logger::log_info(&format!(
            "Updating available pools for liquidity screen: {} pools found",
            pools.len()
        ));

        self.available_pools = pools.clone();

        // Update pool list while preserving focus state
        let was_active = self.pool_dropdown.is_active;
        let was_editing = self.pool_dropdown.is_editing;

        let mut dropdown = SimpleList::new("Select Pool");
        let options: Vec<SimpleListOption> = pools
            .iter()
            .map(|(pool_id, display_name)| {
                SimpleListOption::new(display_name.clone(), pool_id.clone())
            })
            .collect();
        dropdown = dropdown.with_options(options);
        dropdown.set_active(was_active);
        self.pool_dropdown = dropdown;

        crate::tui_dex::utils::logger::log_info("Liquidity pool dropdown updated successfully");
    }

    /// Update current positions
    pub fn update_positions(&mut self, positions: Vec<LiquidityPosition>) {
        self.positions = positions;
        // Reset selection if it's out of bounds
        if let Some(selected) = self.selected_position {
            if selected >= self.positions.len() {
                self.selected_position = None;
            }
        }
    }

    /// Switch operation mode (simplified like swap screen)
    pub fn set_mode(&mut self, mode: LiquidityMode) {
        if self.mode != mode {
            self.mode = mode;
            self.clear_focus();
            // Reset to first input for the new mode
            self.input_focus = LiquidityInputFocus::Pool;
            self.apply_focus();

            crate::tui_dex::utils::logger::log_info(&format!(
                "Liquidity mode switched to {:?}",
                mode
            ));
        }
    }

    /// Move focus to next input (fixed to match swap screen pattern)
    pub fn next_focus(&mut self) {
        self.input_focus = match self.mode {
            LiquidityMode::Provide => match self.input_focus {
                LiquidityInputFocus::Pool => LiquidityInputFocus::FirstAssetAmount,
                LiquidityInputFocus::FirstAssetAmount => LiquidityInputFocus::SecondAssetAmount,
                LiquidityInputFocus::SecondAssetAmount => LiquidityInputFocus::SlippageAmount,
                LiquidityInputFocus::SlippageAmount => LiquidityInputFocus::Execute,
                LiquidityInputFocus::Execute => LiquidityInputFocus::Pool,
                _ => LiquidityInputFocus::Pool,
            },
            LiquidityMode::Withdraw => match self.input_focus {
                LiquidityInputFocus::Pool => LiquidityInputFocus::WithdrawAmount,
                LiquidityInputFocus::WithdrawAmount => LiquidityInputFocus::Execute,
                LiquidityInputFocus::Execute => LiquidityInputFocus::Pool,
                _ => LiquidityInputFocus::Pool,
            },
            LiquidityMode::Positions => LiquidityInputFocus::Pool, // No navigation in positions mode
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Move focus to previous input (fixed to match swap screen pattern)
    pub fn previous_focus(&mut self) {
        self.input_focus = match self.mode {
            LiquidityMode::Provide => match self.input_focus {
                LiquidityInputFocus::Pool => LiquidityInputFocus::Execute,
                LiquidityInputFocus::FirstAssetAmount => LiquidityInputFocus::Pool,
                LiquidityInputFocus::SecondAssetAmount => LiquidityInputFocus::FirstAssetAmount,
                LiquidityInputFocus::SlippageAmount => LiquidityInputFocus::SecondAssetAmount,
                LiquidityInputFocus::Execute => LiquidityInputFocus::SlippageAmount,
                _ => LiquidityInputFocus::Execute,
            },
            LiquidityMode::Withdraw => match self.input_focus {
                LiquidityInputFocus::Pool => LiquidityInputFocus::Execute,
                LiquidityInputFocus::WithdrawAmount => LiquidityInputFocus::Pool,
                LiquidityInputFocus::Execute => LiquidityInputFocus::WithdrawAmount,
                _ => LiquidityInputFocus::Execute,
            },
            LiquidityMode::Positions => LiquidityInputFocus::Pool, // No navigation in positions mode
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Clear focus from all inputs (simplified like swap screen)
    fn clear_focus(&mut self) {
        self.pool_dropdown.set_active(false);
        self.first_asset_input.set_focused(false);
        self.second_asset_input.set_focused(false);
        self.slippage_input.set_focused(false);
        self.withdraw_amount_input.set_focused(false);
    }

    /// Public wrapper to clear all focus states (used by external modules)
    pub fn reset_focus(&mut self) {
        self.clear_focus();
    }

    /// Set focus on current input (simplified like swap screen)
    fn set_focus(&mut self) {
        match self.input_focus {
            LiquidityInputFocus::Pool => self.pool_dropdown.set_active(true),
            LiquidityInputFocus::FirstAssetAmount => self.first_asset_input.set_focused(true),
            LiquidityInputFocus::SecondAssetAmount => self.second_asset_input.set_focused(true),
            LiquidityInputFocus::SlippageAmount => self.slippage_input.set_focused(true),
            LiquidityInputFocus::WithdrawAmount => self.withdraw_amount_input.set_focused(true),
            LiquidityInputFocus::Execute => {} // Button focus handled separately
        }
    }

    /// Public wrapper to apply focus based on `input_focus` value (used by external modules)
    pub fn apply_focus(&mut self) {
        self.clear_focus();
        self.set_focus();
    }

    /// Mark input change for calculations
    pub fn mark_input_change(&mut self) {
        self.last_input_change = Some(std::time::Instant::now());
    }

    /// Update pool reserves from blockchain data
    pub fn update_pool_reserves(&mut self, reserves: Vec<(Uint128, String)>) {
        self.current_pool_reserves = Some(reserves);
        crate::tui_dex::utils::logger::log_info(&format!(
            "Updated pool reserves: {} assets",
            self.current_pool_reserves
                .as_ref()
                .map(|r| r.len())
                .unwrap_or(0)
        ));
    }

    /// Calculate proportional amount for the other asset when one amount is entered
    pub fn calculate_proportional_amount(&mut self, changed_field: LiquidityInputFocus) {
        // Prevent infinite loops during calculation
        if self.updating_proportional_amount {
            return;
        }

        // Only calculate for provide mode with valid pool reserves
        if self.mode != LiquidityMode::Provide {
            return;
        }

        let reserves = match &self.current_pool_reserves {
            Some(reserves) if reserves.len() >= 2 => reserves,
            _ => {
                crate::tui_dex::utils::logger::log_debug(
                    "Cannot calculate proportional amounts: insufficient pool reserves",
                );
                return;
            }
        };

        // Get the pool label to extract asset denoms
        let pool_label = match self.pool_dropdown.get_selected_label() {
            Some(label) => label,
            None => {
                crate::tui_dex::utils::logger::log_debug(
                    "Cannot calculate proportional amounts: no pool selected",
                );
                return;
            }
        };

        // Extract asset symbols from pool label
        let first_token = extract_first_token_from_pool(pool_label);
        let second_token = extract_second_token_from_pool(pool_label);

        // Find the reserves for our two assets
        let (first_reserve, second_reserve) =
            match self.find_reserves_for_tokens(&first_token, &second_token, reserves) {
                Some((r1, r2)) => (r1, r2),
                None => {
                    crate::tui_dex::utils::logger::log_debug(&format!(
                        "Cannot find reserves for tokens {} and {} in pool reserves",
                        first_token, second_token
                    ));
                    return;
                }
            };

        self.updating_proportional_amount = true;

        match changed_field {
            LiquidityInputFocus::FirstAssetAmount => {
                // Calculate second asset amount based on first asset input
                let first_input = self.first_asset_input.value();
                if !first_input.is_empty() && first_input != "0" && first_input != "0.0" {
                    if let Ok(first_amount) = first_input.parse::<f64>() {
                        let ratio = first_amount / self.uint128_to_f64(first_reserve);
                        let second_amount = ratio * self.uint128_to_f64(second_reserve);
                        let second_amount_str = self.format_calculated_amount(second_amount);

                        crate::tui_dex::utils::logger::log_debug(&format!(
                            "Proportional calculation: {} {} -> {} {} (ratio: {:.6})",
                            first_input, first_token, second_amount_str, second_token, ratio
                        ));

                        self.second_asset_input.set_value(&second_amount_str);
                    }
                } else {
                    // Clear the second input if first is empty/zero
                    self.second_asset_input.clear();
                }
            }
            LiquidityInputFocus::SecondAssetAmount => {
                // Calculate first asset amount based on second asset input
                let second_input = self.second_asset_input.value();
                if !second_input.is_empty() && second_input != "0" && second_input != "0.0" {
                    if let Ok(second_amount) = second_input.parse::<f64>() {
                        let ratio = second_amount / self.uint128_to_f64(second_reserve);
                        let first_amount = ratio * self.uint128_to_f64(first_reserve);
                        let first_amount_str = self.format_calculated_amount(first_amount);

                        crate::tui_dex::utils::logger::log_debug(&format!(
                            "Proportional calculation: {} {} -> {} {} (ratio: {:.6})",
                            second_input, second_token, first_amount_str, first_token, ratio
                        ));

                        self.first_asset_input.set_value(&first_amount_str);
                    }
                } else {
                    // Clear the first input if second is empty/zero
                    self.first_asset_input.clear();
                }
            }
            _ => {} // No proportional calculation for other fields
        }

        self.updating_proportional_amount = false;
    }

    /// Find reserves for specific tokens in the reserve list
    fn find_reserves_for_tokens(
        &self,
        first_token: &str,
        second_token: &str,
        reserves: &[(Uint128, String)],
    ) -> Option<(Uint128, Uint128)> {
        let mut first_reserve = None;
        let mut second_reserve = None;

        for (amount, denom) in reserves {
            // Match tokens by checking if the denom contains the token symbol
            // or matches common patterns
            if self.denom_matches_token(denom, first_token) {
                first_reserve = Some(*amount);
            } else if self.denom_matches_token(denom, second_token) {
                second_reserve = Some(*amount);
            }
        }

        match (first_reserve, second_reserve) {
            (Some(r1), Some(r2)) => Some((r1, r2)),
            _ => None,
        }
    }

    /// Check if a denomination matches a token symbol
    fn denom_matches_token(&self, denom: &str, token_symbol: &str) -> bool {
        // Handle various denomination formats
        match token_symbol.to_uppercase().as_str() {
            "OM" => denom == "uom",
            "USDC" => denom.contains("uUSDC") || denom.contains("usdc"),
            "AUSDY" => denom.contains("ausdy"),
            _ => {
                // Generic matching: check if denom contains token symbol (case insensitive)
                denom.to_lowercase().contains(&token_symbol.to_lowercase()) ||
                // Or if token symbol is in the denom path
                denom.split('/').any(|part| part.to_lowercase().contains(&token_symbol.to_lowercase()))
            }
        }
    }

    /// Convert Uint128 to f64 for calculations (with proper decimal handling)
    fn uint128_to_f64(&self, amount: Uint128) -> f64 {
        // Convert from micro units to actual token amount
        // Most tokens use 6 decimals on Mantra network
        amount.u128() as f64 / 1_000_000.0
    }

    /// Format calculated amount to a reasonable number of decimal places
    fn format_calculated_amount(&self, amount: f64) -> String {
        if amount < 0.000001 {
            format!("{:.9}", amount) // Very small amounts need more precision
        } else if amount < 0.01 {
            format!("{:.6}", amount) // Small amounts
        } else if amount < 1.0 {
            format!("{:.4}", amount) // Fractional amounts
        } else {
            format!("{:.2}", amount) // Larger amounts
        }
    }

    /// Check if any list is currently in editing mode
    pub fn is_any_list_editing(&self) -> bool {
        self.pool_dropdown.is_editing
    }

    /// Handle keyboard input using direct key events (simplified like swap screen)
    pub fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        navigation_mode: crate::tui_dex::app::NavigationMode,
    ) -> bool {
        use crossterm::event::KeyCode;

        // Only handle events when in WithinScreen mode
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

        // Log significant key events for liquidity execution
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
            && matches!(self.input_focus, LiquidityInputFocus::Execute)
        {
            crate::tui_dex::utils::logger::log_info("=== LIQUIDITY EXECUTE KEY PRESSED ===");
            crate::tui_dex::utils::logger::log_debug(&format!("Key event: {:?}", key));
            crate::tui_dex::utils::logger::log_debug(&format!(
                "Current focus: {:?}",
                self.input_focus
            ));
        }

        // Handle regular input focus
        match self.input_focus {
            LiquidityInputFocus::Pool => {
                let list_event = self.pool_dropdown.handle_key_event(key);

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
            LiquidityInputFocus::FirstAssetAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.first_asset_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        // Trigger proportional calculation for the other asset
                        self.calculate_proportional_amount(LiquidityInputFocus::FirstAssetAmount);
                        return true;
                    }
                }
                false
            }
            LiquidityInputFocus::SecondAssetAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.second_asset_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        // Trigger proportional calculation for the other asset
                        self.calculate_proportional_amount(LiquidityInputFocus::SecondAssetAmount);
                        return true;
                    }
                }
                false
            }
            LiquidityInputFocus::SlippageAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.slippage_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }
            LiquidityInputFocus::WithdrawAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.withdraw_amount_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }
            LiquidityInputFocus::Execute => {
                // Handle execute button activation
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            self.mark_input_change();
                            crate::tui_dex::utils::logger::log_info(
                                "Liquidity execute button pressed - validation passed",
                            );
                            return true; // Event will be handled by app
                        } else {
                            crate::tui_dex::utils::logger::log_warning(
                                "Liquidity validation failed - please check all fields",
                            );
                        }
                    }
                    _ => {}
                }
                false
            }
        }
    }

    /// Handle keyboard input (legacy method - kept for compatibility)
    pub fn handle_input(&mut self, input: InputRequest) -> bool {
        // This method is kept for backward compatibility with existing code
        // that still uses InputRequest. New code should use handle_key_event.
        match self.input_focus {
            LiquidityInputFocus::FirstAssetAmount => {
                self.first_asset_input.handle_input(input);
                true
            }
            LiquidityInputFocus::SecondAssetAmount => {
                self.second_asset_input.handle_input(input);
                true
            }
            LiquidityInputFocus::SlippageAmount => {
                self.slippage_input.handle_input(input);
                true
            }
            LiquidityInputFocus::WithdrawAmount => {
                self.withdraw_amount_input.handle_input(input);
                true
            }
            _ => false,
        }
    }

    /// Navigate positions list
    pub fn navigate_positions(&mut self, next: bool) {
        if self.positions.is_empty() {
            return;
        }

        match self.selected_position {
            None => {
                self.selected_position = Some(0);
            }
            Some(current) => {
                if next {
                    self.selected_position = Some((current + 1) % self.positions.len());
                } else {
                    self.selected_position = Some(if current == 0 {
                        self.positions.len() - 1
                    } else {
                        current - 1
                    });
                }
            }
        }
    }

    /// Validate current form inputs (simplified like swap screen)
    pub fn validate(&mut self) -> bool {
        match self.mode {
            LiquidityMode::Provide => {
                let pool_valid = self.pool_dropdown.get_selected_value().is_some();
                let first_valid = self.first_asset_input.validate();
                let second_valid = self.second_asset_input.validate();
                let slippage_valid = self.slippage_input.validate();

                pool_valid && first_valid && second_valid && slippage_valid
            }
            LiquidityMode::Withdraw => {
                let pool_valid = self.pool_dropdown.get_selected_value().is_some();
                let withdraw_valid = self.withdraw_amount_input.validate();

                pool_valid && withdraw_valid
            }
            LiquidityMode::Positions => true, // No validation needed for positions view
        }
    }

    /// Get detailed validation errors for user feedback (simplified like swap screen)
    pub fn get_validation_errors(&mut self) -> Vec<String> {
        let mut errors = Vec::new();

        match self.mode {
            LiquidityMode::Provide => {
                if self.pool_dropdown.get_selected_value().is_none() {
                    errors.push("Please select a liquidity pool".to_string());
                }

                if !self.first_asset_input.validate() {
                    if self.first_asset_input.value().is_empty() {
                        errors.push("Please enter first asset amount".to_string());
                    } else {
                        errors.push("Please enter a valid first asset amount".to_string());
                    }
                }

                if !self.second_asset_input.validate() {
                    if self.second_asset_input.value().is_empty() {
                        errors.push("Please enter second asset amount".to_string());
                    } else {
                        errors.push("Please enter a valid second asset amount".to_string());
                    }
                }

                if !self.slippage_input.validate() {
                    errors.push("Please enter valid slippage tolerance (0.1-20%)".to_string());
                }
            }
            LiquidityMode::Withdraw => {
                if self.pool_dropdown.get_selected_value().is_none() {
                    errors.push("Please select a pool to withdraw from".to_string());
                }

                if !self.withdraw_amount_input.validate() {
                    if self.withdraw_amount_input.value().is_empty() {
                        errors.push("Please enter LP token amount to withdraw".to_string());
                    } else {
                        errors.push("Please enter a valid LP token amount".to_string());
                    }
                }
            }
            LiquidityMode::Positions => {
                // No validation needed for positions view
            }
        }

        errors
    }

    /// Show confirmation modal using global app state (like swap screen)
    pub fn show_confirmation_modal(
        &mut self,
        operation_details: &LiquidityOperationDetails,
    ) -> String {
        let message = match self.mode {
            LiquidityMode::Provide => format!(
                "Confirm Provide Liquidity:\n\n• First Asset: {} {}\n• Second Asset: {} {}\n• Pool: {}\n• Expected LP Tokens: {}\n• Slippage: {}%\n\nProceed with transaction?",
                operation_details.first_amount,
                operation_details.first_asset,
                operation_details.second_amount,
                operation_details.second_asset,
                operation_details.pool_name,
                operation_details.expected_lp_tokens.clone().unwrap_or_else(|| "Calculating...".to_string()),
                operation_details.slippage_amount,
            ),
            LiquidityMode::Withdraw => format!(
                "Confirm Withdraw Liquidity:\n\n• LP Token Amount: {}\n• Pool: {}\n• Expected Assets: {}\n\nProceed with transaction?",
                operation_details.withdraw_amount.clone().unwrap_or_default(),
                operation_details.pool_name,
                operation_details.expected_assets.clone().unwrap_or_else(|| "Calculating...".to_string()),
            ),
            _ => "Invalid operation".to_string(),
        };

        // Return the message for the global app to handle
        message
    }

    /// Hide confirmation modal (now handled by global app state)
    pub fn hide_confirmation_modal(&mut self) {
        // Modal state is now managed by the global app
        // This method is kept for compatibility but doesn't do anything
    }
}

/// Liquidity operation details for confirmation (simplified like swap screen)
#[derive(Debug, Clone)]
pub struct LiquidityOperationDetails {
    pub first_amount: String,
    pub first_asset: String,
    pub second_amount: String,
    pub second_asset: String,
    pub pool_name: String,
    pub slippage_amount: String,
    pub expected_lp_tokens: Option<String>,
    pub withdraw_amount: Option<String>,
    pub expected_assets: Option<String>,
}

// Global state for the liquidity screen
static mut LIQUIDITY_SCREEN_STATE: Option<LiquidityScreenState> = None;

pub fn get_liquidity_screen_state() -> &'static mut LiquidityScreenState {
    unsafe { LIQUIDITY_SCREEN_STATE.get_or_insert_with(LiquidityScreenState::default) }
}

/// Main render function for the liquidity screen (simplified like swap screen)
pub fn render_liquidity(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create main layout: header, navigation, content, status
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header, navigation, and status bar
    render_header(f, &app.state, main_chunks[0]);
    render_navigation(f, &app.state, main_chunks[1]);
    render_status_bar(f, &app.state, main_chunks[3]);

    // Render liquidity content
    render_liquidity_content(f, main_chunks[2], app);

    // Render validation overlay if needed
    if app.state.current_screen == crate::tui_dex::app::Screen::Liquidity {
        render_validation_overlay(f, size, app);
    }
}

/// Render the main liquidity content (simplified like swap screen)
fn render_liquidity_content(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    // Create simple tab layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Simple tab titles
    let tabs = vec!["Provide", "Withdraw", "Positions"];
    let tab_index = match liquidity_state.mode {
        LiquidityMode::Provide => 0,
        LiquidityMode::Withdraw => 1,
        LiquidityMode::Positions => 2,
    };

    // Render tabs
    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Liquidity Management"),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(tab_index);

    f.render_widget(tabs_widget, chunks[0]);

    // Render content based on current mode
    match liquidity_state.mode {
        LiquidityMode::Provide => render_provide_liquidity_panel(f, chunks[1], app),
        LiquidityMode::Withdraw => render_withdraw_liquidity_panel(f, chunks[1], app),
        LiquidityMode::Positions => render_positions_panel(f, chunks[1], app),
    }
}

/// Render the provide liquidity panel
fn render_provide_liquidity_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left side: Input form
    render_provide_liquidity_form(f, chunks[0], app);

    // Right side: Preview and expected results
    render_provide_liquidity_preview(f, chunks[1], app);
}

/// Render the provide liquidity form (updated to match swap screen)
fn render_provide_liquidity_form(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(8), // Pool selection (taller for SimpleList)
            Constraint::Length(5), // First asset (proper height for text input)
            Constraint::Length(5), // Second asset (proper height for text input)
            Constraint::Length(5), // Slippage (proper height for text input)
            Constraint::Length(5), // Execute button (proper height for button)
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Provide Liquidity");
    f.render_widget(block, area);

    // Render input fields in order
    let liquidity_state_mut = get_liquidity_screen_state();
    liquidity_state_mut.pool_dropdown.render(f, chunks[0]);

    // Render first asset input with balance display (like swap screen)
    render_first_asset_input_with_balance(f, chunks[1], app);

    // Render second asset input with balance display (like swap screen)
    render_second_asset_input_with_balance(f, chunks[2], app);

    // Render slippage input
    liquidity_state.slippage_input.render(f, chunks[3]);

    // Render execute button (fixed like swap screen)
    render_provide_execute_button(f, chunks[4], app);
}

/// Helper function to render asset input with balance display
fn render_asset_input_with_balance<F>(
    f: &mut Frame,
    area: Rect,
    app: &App,
    input_widget: &TextInput,
    token_extractor: F,
) where
    F: Fn(&str) -> String,
{
    let liquidity_state = get_liquidity_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Render input field
    input_widget.render(f, chunks[0]);

    // Render balance display - extract token from selected pool
    let pool_label = liquidity_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("Select Pool");

    let token = token_extractor(pool_label);

    let balance_text = if token == "Select Pool" {
        vec![
            Line::from(vec![Span::styled(
                "Select pool",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                "to view balance",
                Style::default().fg(Color::Gray),
            )]),
        ]
    } else {
        // Get the balance using the token symbol mapping
        if let Some(micro_balance) = app.get_balance_by_token_name(&token) {
            // Map the token symbol back to denomination to get decimals
            if let Some(denom) = app.map_token_name_to_denom(&token) {
                // Convert from micro units to actual token amount
                let token_amount = app.micro_to_token_amount(micro_balance, &denom);

                vec![
                    Line::from(vec![Span::styled(
                        "Available:",
                        Style::default().fg(Color::Gray),
                    )]),
                    Line::from(vec![Span::styled(
                        format!("{} {}", token_amount, token),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )]),
                ]
            } else {
                // Fallback: show raw balance if denomination mapping fails
                vec![
                    Line::from(vec![Span::styled(
                        "Available:",
                        Style::default().fg(Color::Gray),
                    )]),
                    Line::from(vec![Span::styled(
                        format!("{} {} (raw)", micro_balance, token),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )]),
                ]
            }
        } else {
            // No balance found
            vec![
                Line::from(vec![Span::styled(
                    "Available:",
                    Style::default().fg(Color::Gray),
                )]),
                Line::from(vec![Span::styled(
                    format!("0.0 {}", token),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]),
            ]
        }
    };

    let balance_paragraph = Paragraph::new(Text::from(balance_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Balance")
                .border_style(Style::default().fg(Color::Green)),
        )
        .alignment(Alignment::Center);

    f.render_widget(balance_paragraph, chunks[1]);
}

/// Render first asset input with balance display (like swap screen)
fn render_first_asset_input_with_balance(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();
    render_asset_input_with_balance(
        f,
        area,
        app,
        &liquidity_state.first_asset_input,
        extract_first_token_from_pool,
    );
}

/// Render second asset input with balance display (like swap screen)
fn render_second_asset_input_with_balance(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();
    render_asset_input_with_balance(
        f,
        area,
        app,
        &liquidity_state.second_asset_input,
        extract_second_token_from_pool,
    );
}

/// Extract first token from pool label (like swap screen token extraction)
fn extract_first_token_from_pool(pool_label: &str) -> String {
    if pool_label == "Select Pool" || !pool_label.contains(':') {
        return "Select Pool".to_string();
    }

    // Parse "Pool X: TokenA (amount) / TokenB (amount)" format
    let parts: Vec<&str> = pool_label.split(':').collect();
    if parts.len() >= 2 {
        let token_part = parts[1].trim();
        let tokens: Vec<&str> = token_part.split(" / ").collect();
        if !tokens.is_empty() {
            // Extract token symbol before the parentheses
            let first_token = tokens[0].trim();
            if let Some(paren_pos) = first_token.find('(') {
                first_token[..paren_pos].trim().to_string()
            } else {
                first_token.to_string()
            }
        } else {
            "Unknown".to_string()
        }
    } else {
        "Unknown".to_string()
    }
}

/// Extract second token from pool label (like swap screen token extraction)
fn extract_second_token_from_pool(pool_label: &str) -> String {
    if pool_label == "Select Pool" || !pool_label.contains(':') {
        return "Select Pool".to_string();
    }

    // Parse "Pool X: TokenA (amount) / TokenB (amount)" format
    let parts: Vec<&str> = pool_label.split(':').collect();
    if parts.len() >= 2 {
        let token_part = parts[1].trim();
        let tokens: Vec<&str> = token_part.split(" / ").collect();
        if tokens.len() >= 2 {
            // Extract token symbol before the parentheses
            let second_token = tokens[1].trim();
            if let Some(paren_pos) = second_token.find('(') {
                second_token[..paren_pos].trim().to_string()
            } else {
                second_token.to_string()
            }
        } else {
            "Unknown".to_string()
        }
    } else {
        "Unknown".to_string()
    }
}

/// Render provide execute button (fixed to match swap screen)
fn render_provide_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();
    let is_focused = matches!(liquidity_state.input_focus, LiquidityInputFocus::Execute);
    let is_valid = liquidity_state.clone().validate();

    // Enhanced loading state detection and display (like swap screen)
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
        // Show prominent loading state
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            if loading_message.is_empty() {
                "Processing..."
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
            "► Provide Liquidity ◄",
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "Provide Liquidity",
            Style::default().fg(Color::Green),
        )
    };

    // Add loading indicator if available (like swap screen)
    let button_content = if is_loading {
        // Show animated dots for loading
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

/// Render the provide liquidity preview
fn render_provide_liquidity_preview(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Preview");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let preview_text = if liquidity_state.first_asset_input.value().is_empty()
        || liquidity_state.second_asset_input.value().is_empty()
        || liquidity_state.pool_dropdown.get_selected_value().is_none()
    {
        "Enter amounts and select pool to see preview"
    } else {
        "Calculating expected LP tokens..."
    };

    let expected_lp = liquidity_state
        .expected_lp_tokens
        .as_ref()
        .map(|tokens| {
            format!(
                "Expected LP Tokens: {}",
                format_large_number(&tokens.to_string())
            )
        })
        .unwrap_or_else(|| "Expected LP Tokens: Calculating...".to_string());

    let pool_name = liquidity_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("No pool selected");

    let preview_content = format!(
        "{}\n\n{}\n\nPool: {}\n\nSlippage Tolerance: {}%",
        preview_text,
        expected_lp,
        pool_name,
        liquidity_state.slippage_input.value(),
    );

    let paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render the withdraw liquidity panel
fn render_withdraw_liquidity_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left side: Input form
    render_withdraw_liquidity_form(f, chunks[0], app);

    // Right side: Expected assets
    render_withdraw_liquidity_preview(f, chunks[1], app);
}

/// Render the withdraw liquidity form (simplified like swap screen)
fn render_withdraw_liquidity_form(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(8), // Pool selection (taller for SimpleList)
            Constraint::Length(5), // LP token amount (proper height for text input)
            Constraint::Length(5), // Execute button (proper height for button)
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Withdraw Liquidity");
    f.render_widget(block, area);

    // Render input fields in order
    let liquidity_state_mut = get_liquidity_screen_state();
    liquidity_state_mut.pool_dropdown.render(f, chunks[0]);

    liquidity_state.withdraw_amount_input.render(f, chunks[1]);

    // Render execute button (fixed like swap screen)
    render_withdraw_execute_button(f, chunks[2], app);
}

/// Render the withdraw liquidity preview
fn render_withdraw_liquidity_preview(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Expected Assets");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let preview_text = if liquidity_state.withdraw_amount_input.value().is_empty()
        || liquidity_state.pool_dropdown.get_selected_value().is_none()
    {
        "Enter LP token amount and select pool"
    } else {
        "Calculating expected assets..."
    };

    let expected_assets = liquidity_state
        .expected_assets
        .as_ref()
        .map(|(amount1, amount2, denom1, denom2)| {
            format!(
                "Expected Assets:\n• {}: {}\n• {}: {}",
                denom1,
                format_large_number(&amount1.to_string()),
                denom2,
                format_large_number(&amount2.to_string())
            )
        })
        .unwrap_or_else(|| "Expected Assets: Calculating...".to_string());

    let pool_name = liquidity_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("No pool selected");

    let preview_content = format!(
        "{}\n\n{}\n\nPool: {}",
        preview_text, expected_assets, pool_name,
    );

    let paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render the positions panel
fn render_positions_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Top: Positions table
    render_positions_table(f, chunks[0], app);

    // Bottom: Position details
    render_position_details(f, chunks[1], app);
}

/// Render the current positions table
fn render_positions_table(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Current Positions");

    if liquidity_state.positions.is_empty() {
        let empty_msg =
            Paragraph::new("No liquidity positions found\nProvide liquidity to see positions here")
                .style(Style::default().fg(Color::Gray))
                .block(block)
                .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Pool").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Asset Pair").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("LP Tokens").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Value (USD)").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("PnL").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Share %").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = liquidity_state
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let pnl_color = if position.pnl_percentage >= 0.0 {
                Color::Green
            } else {
                Color::Red
            };

            let pnl_text = format!("{:.2}% (${:.2})", position.pnl_percentage, position.pnl_usd);

            let mut row = Row::new(vec![
                Cell::from(position.pool_id.clone()),
                Cell::from(position.asset_pair.clone()),
                Cell::from(format_large_number(&position.lp_token_amount.to_string())),
                Cell::from(format!("${:.2}", position.estimated_value_usd)),
                Cell::from(pnl_text).style(Style::default().fg(pnl_color)),
                Cell::from(format!("{:.2}%", position.share_percentage)),
            ]);

            // Highlight selected row
            if Some(index) == liquidity_state.selected_position {
                row = row.style(Style::default().add_modifier(Modifier::REVERSED));
            }

            row
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(block);

    f.render_widget(table, area);
}

/// Render detailed information for the selected position
fn render_position_details(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Position Details");

    if let Some(selected_index) = liquidity_state.selected_position {
        if let Some(position) = liquidity_state.positions.get(selected_index) {
            let details = format!(
                "Pool ID: {}\n\nAsset Composition:\n• {}: {}\n• {}: {}\n\nPerformance:\n• Initial Value: ${:.2}\n• Current Value: ${:.2}\n• PnL: {:.2}% (${:.2})\n• Pool Share: {:.2}%\n\nLP Token Balance: {}",
                position.pool_id,
                position.first_asset_denom,
                format_large_number(&position.first_asset_amount.to_string()),
                position.second_asset_denom,
                format_large_number(&position.second_asset_amount.to_string()),
                position.initial_value_usd,
                position.estimated_value_usd,
                position.pnl_percentage,
                position.pnl_usd,
                position.share_percentage,
                format_large_number(&position.lp_token_amount.to_string()),
            );

            let paragraph = Paragraph::new(details)
                .style(Style::default().fg(Color::White))
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        } else {
            let error_msg = Paragraph::new("Invalid position selection")
                .style(Style::default().fg(Color::Red))
                .block(block);
            f.render_widget(error_msg, area);
        }
    } else {
        let instruction_msg = Paragraph::new("Select a position from the table above to view details\n\nUse ↑/↓ arrow keys to navigate")
            .style(Style::default().fg(Color::Gray))
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(instruction_msg, area);
    }
}

/// Handle liquidity screen input
pub fn handle_liquidity_screen_input(input: InputRequest) -> bool {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.handle_input(input)
}

/// Handle liquidity screen navigation
pub fn handle_liquidity_screen_navigation(next: bool) {
    let liquidity_state = get_liquidity_screen_state();

    match liquidity_state.mode {
        LiquidityMode::Positions => {
            liquidity_state.navigate_positions(next);
        }
        _ => {
            if next {
                liquidity_state.next_focus();
            } else {
                liquidity_state.previous_focus();
            }
        }
    }
}

/// Switch liquidity mode (simplified like swap screen)
pub fn switch_liquidity_mode(mode: LiquidityMode) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.set_mode(mode);
}

/// Execute liquidity operation with confirmation (enhanced to match swap screen)
pub fn execute_liquidity_operation_with_confirmation() {
    let liquidity_state = get_liquidity_screen_state();

    crate::tui_dex::utils::logger::log_info("=== LIQUIDITY EXECUTION ATTEMPT ===");

    if !liquidity_state.validate() {
        let errors = liquidity_state.clone().get_validation_errors();
        crate::tui_dex::utils::logger::log_error("Liquidity validation failed:");
        for error in &errors {
            crate::tui_dex::utils::logger::log_error(&format!("  - {}", error));
        }
        return;
    }

    let operation_details = match liquidity_state.mode {
        LiquidityMode::Provide => {
            // Get current values from the form
            let first_amount = liquidity_state.first_asset_input.value();
            let second_amount = liquidity_state.second_asset_input.value();
            let pool_id_str = liquidity_state
                .pool_dropdown
                .get_selected_value()
                .unwrap_or_default();
            let slippage = liquidity_state.slippage_input.value();

            // Get pool name for asset extraction
            let pool_name = liquidity_state
                .pool_dropdown
                .get_selected_label()
                .unwrap_or("Unknown Pool");

            // Extract asset names from pool label
            let (first_asset, second_asset) = extract_assets_from_pool_label(pool_name);

            // Log liquidity parameters
            crate::tui_dex::utils::logger::log_info("Provide Liquidity parameters:");
            crate::tui_dex::utils::logger::log_info(&format!(
                "  First Amount: {} {}",
                first_amount, first_asset
            ));
            crate::tui_dex::utils::logger::log_info(&format!(
                "  Second Amount: {} {}",
                second_amount, second_asset
            ));
            crate::tui_dex::utils::logger::log_info(&format!("  Pool ID: {}", pool_id_str));
            crate::tui_dex::utils::logger::log_info(&format!(
                "  Slippage Tolerance: {}%",
                slippage
            ));

            LiquidityOperationDetails {
                first_amount: first_amount.to_string(),
                first_asset,
                second_amount: second_amount.to_string(),
                second_asset,
                pool_name: pool_name.to_string(),
                slippage_amount: slippage.to_string(),
                expected_lp_tokens: liquidity_state
                    .expected_lp_tokens
                    .as_ref()
                    .map(|t| t.to_string()),
                withdraw_amount: None,
                expected_assets: None,
            }
        }
        LiquidityMode::Withdraw => {
            // Get current values from the form
            let lp_amount = liquidity_state.withdraw_amount_input.value();
            let pool_id_str = liquidity_state
                .pool_dropdown
                .get_selected_value()
                .unwrap_or_default();

            // Get pool name
            let pool_name = liquidity_state
                .pool_dropdown
                .get_selected_label()
                .unwrap_or("Unknown Pool");

            // Log withdraw parameters
            crate::tui_dex::utils::logger::log_info("Withdraw Liquidity parameters:");
            crate::tui_dex::utils::logger::log_info(&format!("  LP Token Amount: {}", lp_amount));
            crate::tui_dex::utils::logger::log_info(&format!("  Pool ID: {}", pool_id_str));

            LiquidityOperationDetails {
                first_amount: String::new(),
                first_asset: String::new(),
                second_amount: String::new(),
                second_asset: String::new(),
                pool_name: pool_name.to_string(),
                slippage_amount: String::new(),
                expected_lp_tokens: None,
                withdraw_amount: Some(lp_amount.to_string()),
                expected_assets: liquidity_state
                    .expected_assets
                    .as_ref()
                    .map(|(a1, a2, d1, d2)| format!("{} {}, {} {}", a1, d1, a2, d2)),
            }
        }
        LiquidityMode::Positions => {
            crate::tui_dex::utils::logger::log_warning("No execution operation for positions view");
            return;
        }
    };

    let confirmation_message = liquidity_state.show_confirmation_modal(&operation_details);

    crate::tui_dex::utils::logger::log_info("Liquidity confirmation modal prepared");
    crate::tui_dex::utils::logger::log_debug(&format!(
        "Confirmation message: {}",
        confirmation_message
    ));
    crate::tui_dex::utils::logger::log_info(&format!(
        "Liquidity confirmation ready: {}",
        confirmation_message
    ));
}

/// Extract asset names from pool label (similar to swap screen token extraction)
pub fn extract_assets_from_pool_label(pool_label: &str) -> (String, String) {
    if !pool_label.contains(':') {
        return ("Unknown".to_string(), "Unknown".to_string());
    }

    // Parse "Pool X: TokenA (amount) / TokenB (amount)" format
    let parts: Vec<&str> = pool_label.split(':').collect();
    if parts.len() >= 2 {
        let token_part = parts[1].trim();
        let tokens: Vec<&str> = token_part.split(" / ").collect();
        if tokens.len() >= 2 {
            // Extract token symbols before the parentheses
            let first_token = tokens[0].trim();
            let first_asset = if let Some(paren_pos) = first_token.find('(') {
                first_token[..paren_pos].trim().to_string()
            } else {
                first_token.to_string()
            };

            let second_token = tokens[1].trim();
            let second_asset = if let Some(paren_pos) = second_token.find('(') {
                second_token[..paren_pos].trim().to_string()
            } else {
                second_token.to_string()
            };

            return (first_asset, second_asset);
        }
    }

    ("Unknown".to_string(), "Unknown".to_string())
}

/// Handle confirmation response (like swap screen)
pub fn handle_liquidity_confirmation_response(
    confirmed: bool,
) -> Option<crate::tui_dex::events::Event> {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.hide_confirmation_modal();

    crate::tui_dex::utils::logger::log_info(&format!(
        "=== LIQUIDITY CONFIRMATION RESPONSE: {} ===",
        if confirmed { "CONFIRMED" } else { "CANCELLED" }
    ));

    if confirmed {
        // Create the appropriate liquidity event based on mode
        match liquidity_state.mode {
            LiquidityMode::Provide => {
                // Return ProvideLiquidity event
                let first_amount = liquidity_state.first_asset_input.value();
                let second_amount = liquidity_state.second_asset_input.value();
                let pool_id_str = liquidity_state
                    .pool_dropdown
                    .get_selected_value()
                    .unwrap_or_default();
                let slippage = liquidity_state.slippage_input.value();

                // Use pool_id as string (no parsing needed like swap screen)
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Using pool ID as string: '{}'",
                    pool_id_str
                ));

                // Validate that we have a valid pool ID before proceeding
                if pool_id_str.is_empty() {
                    crate::tui_dex::utils::logger::log_error(
                        "Liquidity execution failed: No pool selected",
                    );
                    return None;
                }

                let event = crate::tui_dex::events::Event::ProvideLiquidity {
                    asset_1_amount: first_amount.to_string(),
                    asset_2_amount: second_amount.to_string(),
                    pool_id: pool_id_str.to_string(),
                    slippage_tolerance: Some(slippage.to_string()),
                };

                crate::tui_dex::utils::logger::log_info(&format!(
                    "Created ProvideLiquidity event: asset_1={}, asset_2={}, pool_id={}, slippage={:?}",
                    first_amount, second_amount, pool_id_str, slippage
                ));

                Some(event)
            }
            LiquidityMode::Withdraw => {
                // Return WithdrawLiquidity event
                let lp_amount = liquidity_state.withdraw_amount_input.value();
                let pool_id_str = liquidity_state
                    .pool_dropdown
                    .get_selected_value()
                    .unwrap_or_default();

                // Use pool_id as string (no parsing needed like swap screen)
                if pool_id_str.is_empty() {
                    crate::tui_dex::utils::logger::log_error(
                        "Withdraw execution failed: No pool selected",
                    );
                    return None;
                }

                Some(crate::tui_dex::events::Event::WithdrawLiquidity {
                    lp_token_amount: lp_amount.to_string(),
                    pool_id: pool_id_str.to_string(),
                    slippage_tolerance: None, // Optional for withdraw
                })
            }
            LiquidityMode::Positions => None, // No operation for positions view
        }
    } else {
        None
    }
}

/// Reset liquidity forms (like swap screen)
pub fn reset_liquidity_forms() {
    let liquidity_state = get_liquidity_screen_state();

    // Preserve pool data before reset
    let available_pools = liquidity_state.available_pools.clone();

    // Reset form inputs
    liquidity_state.first_asset_input.clear();
    liquidity_state.second_asset_input.clear();
    liquidity_state.withdraw_amount_input.clear();
    liquidity_state.expected_lp_tokens = None;
    liquidity_state.expected_assets = None;

    // Restore pool data
    liquidity_state.update_available_pools(available_pools);

    crate::tui_dex::utils::logger::log_info("Liquidity forms reset completed");
}

/// Update expected LP tokens from calculation
pub fn update_expected_lp_tokens(amount: Uint128) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.expected_lp_tokens = Some(amount);
}

/// Update expected assets from withdrawal calculation
pub fn update_expected_assets(amount1: Uint128, amount2: Uint128, denom1: String, denom2: String) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.expected_assets = Some((amount1, amount2, denom1, denom2));
}

/// Update liquidity positions
pub fn update_liquidity_positions(positions: Vec<LiquidityPosition>) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.update_positions(positions);
}

/// Update available pools for liquidity operations
pub fn update_liquidity_pools(pools: Vec<(String, String)>) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.update_available_pools(pools);
}

/// Update pool reserves for proportional calculations
pub fn update_liquidity_pool_reserves(reserves: Vec<(Uint128, String)>) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.update_pool_reserves(reserves);
}

/// Initialize focus for the liquidity screen (called when entering the screen)
pub fn initialize_liquidity_screen_focus() {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.input_focus = LiquidityInputFocus::Pool;
    liquidity_state.apply_focus();

    crate::tui_dex::utils::logger::log_info("Liquidity screen focus initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_screen_state_navigation() {
        let mut state = LiquidityScreenState::default();

        // Test provide mode navigation
        assert_eq!(state.input_focus, LiquidityInputFocus::Pool);
        state.next_focus();
        assert_eq!(state.input_focus, LiquidityInputFocus::FirstAssetAmount);
        state.next_focus();
        assert_eq!(state.input_focus, LiquidityInputFocus::SecondAssetAmount);

        // Test mode switching
        state.set_mode(LiquidityMode::Withdraw);
        assert_eq!(state.mode, LiquidityMode::Withdraw);
        assert_eq!(state.input_focus, LiquidityInputFocus::Pool);
    }

    #[test]
    fn test_liquidity_validation() {
        let mut state = LiquidityScreenState::default();

        // Test provide mode validation
        assert!(!state.validate()); // Should fail with empty inputs

        state.first_asset_input = state.first_asset_input.with_value("100");
        state.second_asset_input = state.second_asset_input.with_value("200");
        // Still should fail without pool selection
        assert!(!state.validate());
    }

    #[test]
    fn test_position_navigation() {
        let mut state = LiquidityScreenState::default();
        state.set_mode(LiquidityMode::Positions);

        // Add test positions
        let positions = vec![
            LiquidityPosition {
                pool_id: "1".to_string(),
                asset_pair: "USDC/USDT".to_string(),
                lp_token_amount: Uint128::new(1000),
                estimated_value_usd: 1000.0,
                initial_value_usd: 950.0,
                pnl_percentage: 5.26,
                pnl_usd: 50.0,
                share_percentage: 0.1,
                first_asset_amount: Uint128::new(500),
                second_asset_amount: Uint128::new(500),
                first_asset_denom: "USDC".to_string(),
                second_asset_denom: "USDT".to_string(),
            },
            LiquidityPosition {
                pool_id: "2".to_string(),
                asset_pair: "ATOM/OSMO".to_string(),
                lp_token_amount: Uint128::new(2000),
                estimated_value_usd: 2000.0,
                initial_value_usd: 2100.0,
                pnl_percentage: -4.76,
                pnl_usd: -100.0,
                share_percentage: 0.2,
                first_asset_amount: Uint128::new(1000),
                second_asset_amount: Uint128::new(1000),
                first_asset_denom: "ATOM".to_string(),
                second_asset_denom: "OSMO".to_string(),
            },
        ];

        state.update_positions(positions);

        // Test navigation
        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(0));

        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(1));

        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(0)); // Should wrap around
    }
}

/// Render validation error overlay for immediate feedback
fn render_validation_overlay(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    // Only show validation errors when the execute button is focused and validation fails
    if !matches!(liquidity_state.input_focus, LiquidityInputFocus::Execute) {
        return;
    }

    if liquidity_state.clone().validate() {
        return; // No errors to display
    }

    let errors = liquidity_state.clone().get_validation_errors();
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
    f.render_widget(ratatui::widgets::Clear, overlay_area);

    // Create error content
    let error_lines: Vec<ratatui::text::Line> = errors
        .iter()
        .map(|error| {
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("• ", Style::default().fg(Color::Red)),
                ratatui::text::Span::styled(error, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let error_text = ratatui::text::Text::from(error_lines);

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

/// Render withdraw execute button (fixed to match swap screen)
fn render_withdraw_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();
    let is_focused = matches!(liquidity_state.input_focus, LiquidityInputFocus::Execute);
    let is_valid = liquidity_state.clone().validate();

    // Enhanced loading state detection and display (like swap screen)
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
        // Show prominent loading state
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            if loading_message.is_empty() {
                "Processing..."
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
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
            "► Withdraw Liquidity ◄",
            Style::default().fg(Color::Red),
        )
    } else {
        (
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            "Withdraw Liquidity",
            Style::default().fg(Color::Red),
        )
    };

    // Add loading indicator if available (like swap screen)
    let button_content = if is_loading {
        // Show animated dots for loading
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

/// Helper function to format large numbers
fn format_large_number(number_str: &str) -> String {
    // Simple formatting - just return the string for now
    // In a real implementation, this would format with commas, etc.
    number_str.to_string()
}
