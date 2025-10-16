//! Multi-hop Swap Screen Implementation
//!
//! This module provides the multi-hop swap interface for the MANTRA DEX SDK TUI,
//! allowing users to build complex swap routes with multiple hops, analyze the
//! complete route with price impact and fees, and execute multi-hop transactions.

use crate::tui_dex::{
    app::{App, LoadingState},
    components::{
        forms::{Dropdown, InputType, TextInput},
        header::render_header,
        modals::{render_modal, ModalState},
        navigation::render_navigation,
        status_bar::render_status_bar,
    },
    events::SwapOperation,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
    Frame,
};
use tui_input::InputRequest;

/// Input focus states for the multi-hop swap screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MultiHopInputFocus {
    FromToken,
    ToToken,
    Amount,
    Pool,
    AddHop,
    RemoveHop,
    Execute,
    RouteList,
}

/// A single hop in the multi-hop swap route
#[derive(Debug, Clone)]
pub struct SwapHop {
    pub from_asset: String,
    pub to_asset: String,
    pub pool_id: String,
    pub pool_name: String,
    pub amount_in: String,
    pub estimated_amount_out: String,
    pub price_impact: f64,
    pub fee_amount: String,
    pub fee_rate: f64,
}

impl Default for SwapHop {
    fn default() -> Self {
        Self {
            from_asset: String::new(),
            to_asset: String::new(),
            pool_id: String::new(),
            pool_name: String::new(),
            amount_in: String::new(),
            estimated_amount_out: String::new(),
            price_impact: 0.0,
            fee_amount: String::new(),
            fee_rate: 0.3, // Default 0.3% fee
        }
    }
}

/// Route analysis summary
#[derive(Debug, Clone)]
pub struct RouteAnalysis {
    pub total_hops: usize,
    pub initial_amount: String,
    pub final_estimated_amount: String,
    pub total_price_impact: f64,
    pub total_fees: String,
    pub estimated_execution_time: String,
    pub slippage_tolerance: f64,
    pub route_efficiency: f64, // 0-100 score
}

impl Default for RouteAnalysis {
    fn default() -> Self {
        Self {
            total_hops: 0,
            initial_amount: "0".to_string(),
            final_estimated_amount: "0".to_string(),
            total_price_impact: 0.0,
            total_fees: "0".to_string(),
            estimated_execution_time: "~30s".to_string(),
            slippage_tolerance: 2.0, // Default 2% for multi-hop
            route_efficiency: 100.0,
        }
    }
}

/// Current multi-hop swap screen state
#[derive(Debug, Clone)]
pub struct MultiHopScreenState {
    /// Current input focus
    pub input_focus: MultiHopInputFocus,
    /// From token dropdown for new hop
    pub from_token_dropdown: Dropdown<String>,
    /// To token dropdown for new hop
    pub to_token_dropdown: Dropdown<String>,
    /// Amount input for new hop
    pub amount_input: TextInput,
    /// Pool selection dropdown for new hop
    pub pool_dropdown: Dropdown<String>,
    /// Current swap route (list of hops)
    pub route: Vec<SwapHop>,
    /// Route list state for navigation
    pub route_list_state: ListState,
    /// Route analysis data
    pub route_analysis: RouteAnalysis,
    /// Whether confirmation modal is shown
    pub show_confirmation: bool,
    /// Modal state for confirmations
    pub modal_state: Option<ModalState>,
    /// Available tokens for selection
    pub available_tokens: Vec<String>,
    /// Available pools for current token pair
    pub available_pools: Vec<(String, String)>, // (pool_id, display_name)
    /// Whether route optimization is enabled
    pub auto_optimize: bool,
    /// Slippage tolerance input
    pub slippage_input: TextInput,
}

impl Default for MultiHopScreenState {
    fn default() -> Self {
        let mut from_token_dropdown = Dropdown::new("From Token").required();
        let to_token_dropdown = Dropdown::new("To Token").required();
        let pool_dropdown = Dropdown::new("Select Pool").required();

        let amount_input = TextInput::new("Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let slippage_input = TextInput::new("Slippage Tolerance (%)")
            .with_type(InputType::Amount)
            .with_value("2.0")
            .with_placeholder("2.0");

        // Set initial focus
        from_token_dropdown.set_focused(true);

        // Start with empty dropdowns - will be populated when real data loads
        let available_tokens = Vec::new();

        Self {
            input_focus: MultiHopInputFocus::FromToken,
            from_token_dropdown,
            to_token_dropdown,
            amount_input,
            pool_dropdown,
            route: Vec::new(),
            route_list_state: ListState::default(),
            route_analysis: RouteAnalysis::default(),
            show_confirmation: false,
            modal_state: None,
            available_tokens,
            available_pools: Vec::new(),
            auto_optimize: true,
            slippage_input,
        }
    }
}

impl MultiHopScreenState {
    /// Add a new hop to the route
    pub fn add_hop(&mut self) {
        if let (Some(from_token), Some(to_token), Some(pool_id)) = (
            self.from_token_dropdown.selected_value(),
            self.to_token_dropdown.selected_value(),
            self.pool_dropdown.selected_value(),
        ) {
            let amount = if self.route.is_empty() {
                self.amount_input.value().to_string()
            } else {
                // Use output from previous hop
                self.route.last().unwrap().estimated_amount_out.clone()
            };

            let mut hop = SwapHop {
                from_asset: from_token.to_string(),
                to_asset: to_token.to_string(),
                pool_id: pool_id.clone(),
                pool_name: self.find_pool_name(pool_id),
                amount_in: amount,
                ..Default::default()
            };

            // Calculate estimates (simplified for demo)
            hop.estimated_amount_out = self.calculate_hop_output(&hop);
            hop.price_impact = self.calculate_hop_price_impact(&hop);
            hop.fee_amount = self.calculate_hop_fee(&hop);

            self.route.push(hop);
            self.update_route_analysis();
            self.prepare_next_hop();
        }
    }

    /// Remove the selected hop from the route
    pub fn remove_selected_hop(&mut self) {
        if let Some(selected) = self.route_list_state.selected() {
            if selected < self.route.len() {
                self.route.remove(selected);
                // Adjust selection
                if self.route.is_empty() {
                    self.route_list_state.select(None);
                } else if selected >= self.route.len() {
                    self.route_list_state.select(Some(self.route.len() - 1));
                }
                self.update_route_analysis();
            }
        }
    }

    /// Clear the entire route
    pub fn clear_route(&mut self) {
        self.route.clear();
        self.route_list_state.select(None);
        self.update_route_analysis();
    }

    /// Prepare inputs for next hop
    fn prepare_next_hop(&mut self) {
        if let Some(last_hop) = self.route.last() {
            // Set from token to the output of the last hop
            self.from_token_dropdown.select_by_value(&last_hop.to_asset);
            // Clear to token selection
            self.to_token_dropdown.clear_selection();
            // Clear amount (will use output from previous hop)
            self.amount_input.clear();
        }
    }

    /// Update route analysis based on current route
    fn update_route_analysis(&mut self) {
        self.route_analysis.total_hops = self.route.len();

        if self.route.is_empty() {
            self.route_analysis = RouteAnalysis::default();
            return;
        }

        // Calculate totals
        self.route_analysis.initial_amount = self.route.first().unwrap().amount_in.clone();
        self.route_analysis.final_estimated_amount =
            self.route.last().unwrap().estimated_amount_out.clone();

        // Sum up price impacts (simplified calculation)
        self.route_analysis.total_price_impact =
            self.route.iter().map(|hop| hop.price_impact).sum();

        // Calculate total fees
        let total_fees: f64 = self
            .route
            .iter()
            .filter_map(|hop| hop.fee_amount.parse::<f64>().ok())
            .sum();
        self.route_analysis.total_fees = format!("{:.6}", total_fees);

        // Estimate execution time (30s base + 10s per hop)
        let time_estimate = 30 + (self.route.len() * 10);
        self.route_analysis.estimated_execution_time = format!("~{}s", time_estimate);

        // Calculate route efficiency (simplified)
        self.route_analysis.route_efficiency = if self.route_analysis.total_price_impact > 0.0 {
            (100.0 - (self.route_analysis.total_price_impact * 10.0)).max(0.0)
        } else {
            100.0
        };
    }

    /// Calculate estimated output for a hop (simplified)
    fn calculate_hop_output(&self, hop: &SwapHop) -> String {
        if let Ok(amount) = hop.amount_in.parse::<f64>() {
            // Simplified calculation: 0.3% fee + some price impact
            let fee_deduction = amount * hop.fee_rate / 100.0;
            let price_impact_loss = amount * hop.price_impact / 100.0;
            let output = amount - fee_deduction - price_impact_loss;
            format!("{:.6}", output.max(0.0))
        } else {
            "0".to_string()
        }
    }

    /// Calculate price impact for a hop (simplified)
    fn calculate_hop_price_impact(&self, _hop: &SwapHop) -> f64 {
        // Simplified: random small impact between 0.1% and 2.0%
        0.1 + (self.route.len() as f64 * 0.3) // Increases with route length
    }

    /// Calculate fee for a hop (simplified)
    fn calculate_hop_fee(&self, hop: &SwapHop) -> String {
        if let Ok(amount) = hop.amount_in.parse::<f64>() {
            let fee = amount * hop.fee_rate / 100.0;
            format!("{:.6}", fee)
        } else {
            "0".to_string()
        }
    }

    /// Find pool name by ID
    fn find_pool_name(&self, pool_id: &str) -> String {
        self.available_pools
            .iter()
            .find(|(id, _)| id == pool_id)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("Pool {}", pool_id))
    }

    /// Move focus to next input
    pub fn next_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.input_focus {
            MultiHopInputFocus::FromToken => MultiHopInputFocus::ToToken,
            MultiHopInputFocus::ToToken => MultiHopInputFocus::Amount,
            MultiHopInputFocus::Amount => MultiHopInputFocus::Pool,
            MultiHopInputFocus::Pool => MultiHopInputFocus::AddHop,
            MultiHopInputFocus::AddHop => MultiHopInputFocus::RouteList,
            MultiHopInputFocus::RouteList => MultiHopInputFocus::RemoveHop,
            MultiHopInputFocus::RemoveHop => MultiHopInputFocus::Execute,
            MultiHopInputFocus::Execute => MultiHopInputFocus::FromToken,
        };
        self.set_focus();
    }

    /// Move focus to previous input
    pub fn previous_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.input_focus {
            MultiHopInputFocus::FromToken => MultiHopInputFocus::Execute,
            MultiHopInputFocus::ToToken => MultiHopInputFocus::FromToken,
            MultiHopInputFocus::Amount => MultiHopInputFocus::ToToken,
            MultiHopInputFocus::Pool => MultiHopInputFocus::Amount,
            MultiHopInputFocus::AddHop => MultiHopInputFocus::Pool,
            MultiHopInputFocus::RouteList => MultiHopInputFocus::AddHop,
            MultiHopInputFocus::RemoveHop => MultiHopInputFocus::RouteList,
            MultiHopInputFocus::Execute => MultiHopInputFocus::RemoveHop,
        };
        self.set_focus();
    }

    /// Clear focus from all inputs
    fn clear_focus(&mut self) {
        self.from_token_dropdown.set_focused(false);
        self.to_token_dropdown.set_focused(false);
        self.amount_input.set_focused(false);
        self.pool_dropdown.set_focused(false);
    }

    /// Set focus on current input
    fn set_focus(&mut self) {
        match self.input_focus {
            MultiHopInputFocus::FromToken => self.from_token_dropdown.set_focused(true),
            MultiHopInputFocus::ToToken => self.to_token_dropdown.set_focused(true),
            MultiHopInputFocus::Amount => self.amount_input.set_focused(true),
            MultiHopInputFocus::Pool => self.pool_dropdown.set_focused(true),
            _ => {} // Other focuses handled separately
        }
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, input: InputRequest) -> bool {
        match self.input_focus {
            MultiHopInputFocus::FromToken => match input {
                InputRequest::GoToPrevWord => {
                    self.from_token_dropdown.move_up();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.from_token_dropdown.move_down();
                    true
                }
                InputRequest::GoToStart => {
                    self.from_token_dropdown.toggle();
                    true
                }
                InputRequest::GoToEnd => {
                    self.from_token_dropdown.select_current();
                    true
                }
                _ => false,
            },
            MultiHopInputFocus::ToToken => match input {
                InputRequest::GoToPrevWord => {
                    self.to_token_dropdown.move_up();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.to_token_dropdown.move_down();
                    true
                }
                InputRequest::GoToStart => {
                    self.to_token_dropdown.toggle();
                    true
                }
                InputRequest::GoToEnd => {
                    self.to_token_dropdown.select_current();
                    true
                }
                _ => false,
            },
            MultiHopInputFocus::Amount => {
                self.amount_input.handle_input(input);
                true
            }
            MultiHopInputFocus::Pool => match input {
                InputRequest::GoToPrevWord => {
                    self.pool_dropdown.move_up();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.pool_dropdown.move_down();
                    true
                }
                InputRequest::GoToStart => {
                    self.pool_dropdown.toggle();
                    true
                }
                InputRequest::GoToEnd => {
                    self.pool_dropdown.select_current();
                    true
                }
                _ => false,
            },
            MultiHopInputFocus::RouteList => match input {
                InputRequest::GoToPrevWord => {
                    self.route_list_select_previous();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.route_list_select_next();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Move route list selection up
    fn route_list_select_previous(&mut self) {
        if self.route.is_empty() {
            return;
        }
        let selected = self.route_list_state.selected().unwrap_or(0);
        if selected > 0 {
            self.route_list_state.select(Some(selected - 1));
        } else {
            self.route_list_state.select(Some(self.route.len() - 1));
        }
    }

    /// Move route list selection down
    fn route_list_select_next(&mut self) {
        if self.route.is_empty() {
            return;
        }
        let selected = self.route_list_state.selected().unwrap_or(0);
        if selected + 1 < self.route.len() {
            self.route_list_state.select(Some(selected + 1));
        } else {
            self.route_list_state.select(Some(0));
        }
    }

    /// Validate the current hop inputs
    pub fn validate_current_hop(&self) -> bool {
        self.from_token_dropdown.selected_value().is_some()
            && self.to_token_dropdown.selected_value().is_some()
            && self.pool_dropdown.selected_value().is_some()
            && (self.route.is_empty() && !self.amount_input.value().is_empty()
                || !self.route.is_empty())
    }

    /// Validate the entire route for execution
    pub fn validate_route(&self) -> bool {
        !self.route.is_empty()
    }

    /// Show confirmation modal for route execution
    pub fn show_confirmation_modal(&mut self) {
        let modal_text = format!(
            "Execute Multi-Hop Swap?\n\n\
            Route: {} hops\n\
            Initial Amount: {} {}\n\
            Final Estimated: {} {}\n\
            Total Price Impact: {:.2}%\n\
            Total Fees: {}\n\
            Slippage Tolerance: {:.1}%\n\n\
            This operation cannot be undone.",
            self.route_analysis.total_hops,
            self.route_analysis.initial_amount,
            self.route
                .first()
                .map(|h| &h.from_asset)
                .unwrap_or(&"".to_string()),
            self.route_analysis.final_estimated_amount,
            self.route
                .last()
                .map(|h| &h.to_asset)
                .unwrap_or(&"".to_string()),
            self.route_analysis.total_price_impact,
            self.route_analysis.total_fees,
            self.route_analysis.slippage_tolerance
        );

        self.modal_state = Some(ModalState::confirmation(
            "Confirm Multi-Hop Swap".to_string(),
            modal_text,
            Some("Execute".to_string()),
            Some("Cancel".to_string()),
        ));
        self.show_confirmation = true;
    }

    /// Hide confirmation modal
    pub fn hide_confirmation_modal(&mut self) {
        self.show_confirmation = false;
        self.modal_state = None;
    }

    /// Get swap operations for execution
    pub fn get_swap_operations(&self) -> Vec<SwapOperation> {
        self.route
            .iter()
            .map(|hop| SwapOperation {
                from_asset: hop.from_asset.clone(),
                to_asset: hop.to_asset.clone(),
                pool_id: hop.pool_id.clone(),
                amount: hop.amount_in.clone(),
            })
            .collect()
    }
}

// Global state for the multi-hop screen
static mut MULTIHOP_SCREEN_STATE: Option<MultiHopScreenState> = None;

/// Get or initialize the multi-hop screen state
fn get_multihop_screen_state() -> &'static mut MultiHopScreenState {
    unsafe {
        if MULTIHOP_SCREEN_STATE.is_none() {
            MULTIHOP_SCREEN_STATE = Some(MultiHopScreenState::default());
        }
        MULTIHOP_SCREEN_STATE.as_mut().unwrap()
    }
}

/// Main render function for the multi-hop swap screen
pub fn render_multihop(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create main layout: header, nav, content, status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header and navigation
    render_header(f, &app.state, chunks[0]);
    render_navigation(f, &app.state, chunks[1]);

    // Render main content
    render_multihop_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);

    // Render modal if shown
    let state = get_multihop_screen_state();
    if state.show_confirmation {
        if let Some(modal_state) = &state.modal_state {
            render_modal(f, modal_state, size);
        }
    }
}

/// Render the main multi-hop content area
fn render_multihop_content(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Route builder panel
            Constraint::Percentage(60), // Route analysis and list panel
        ])
        .split(area);

    render_route_builder(f, chunks[0], app);
    render_route_analysis(f, chunks[1], app);
}

/// Render the route builder panel
fn render_route_builder(f: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .title("Route Builder")
        .borders(Borders::ALL)
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // From token
            Constraint::Length(3), // To token
            Constraint::Length(3), // Amount
            Constraint::Length(3), // Pool selection
            Constraint::Length(3), // Add hop button
            Constraint::Min(1),    // Spacer
            Constraint::Length(3), // Slippage
        ])
        .split(inner);

    let state = get_multihop_screen_state();

    // From token dropdown
    let from_style = if matches!(state.input_focus, MultiHopInputFocus::FromToken) {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let from_token_text = state
        .from_token_dropdown
        .selected_value()
        .cloned()
        .unwrap_or("Select From Token".to_string());
    let from_token = Paragraph::new(from_token_text)
        .block(Block::default().borders(Borders::ALL).title("From Token"))
        .style(from_style);
    f.render_widget(from_token, chunks[0]);

    // To token dropdown
    let to_style = if matches!(state.input_focus, MultiHopInputFocus::ToToken) {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let to_token_text = state
        .to_token_dropdown
        .selected_value()
        .cloned()
        .unwrap_or("Select To Token".to_string());
    let to_token = Paragraph::new(to_token_text)
        .block(Block::default().borders(Borders::ALL).title("To Token"))
        .style(to_style);
    f.render_widget(to_token, chunks[1]);

    // Amount input (only for first hop)
    let amount_style = if matches!(state.input_focus, MultiHopInputFocus::Amount) {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let amount_text = if state.route.is_empty() {
        state.amount_input.value()
    } else {
        "Auto (from previous hop)"
    };
    let amount = Paragraph::new(amount_text)
        .block(Block::default().borders(Borders::ALL).title("Amount"))
        .style(amount_style);
    f.render_widget(amount, chunks[2]);

    // Pool selection dropdown
    let pool_style = if matches!(state.input_focus, MultiHopInputFocus::Pool) {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let pool_text = state
        .pool_dropdown
        .selected_value()
        .cloned()
        .unwrap_or("Select Pool".to_string());
    let pool = Paragraph::new(pool_text)
        .block(Block::default().borders(Borders::ALL).title("Pool"))
        .style(pool_style);
    f.render_widget(pool, chunks[3]);

    // Add hop button
    let add_style = if matches!(state.input_focus, MultiHopInputFocus::AddHop) {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if state.validate_current_hop() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let add_button_text = if state.validate_current_hop() {
        "Press Enter to Add Hop"
    } else {
        "Fill all fields to add hop"
    };
    let add_button = Paragraph::new(add_button_text)
        .block(Block::default().borders(Borders::ALL).title("Add Hop"))
        .style(add_style)
        .alignment(Alignment::Center);
    f.render_widget(add_button, chunks[4]);

    // Slippage tolerance
    let slippage_text = format!("{}%", state.route_analysis.slippage_tolerance);
    let slippage = Paragraph::new(slippage_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Slippage Tolerance"),
        )
        .style(Style::default());
    f.render_widget(slippage, chunks[6]);
}

/// Render the route analysis panel
fn render_route_analysis(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Route list
            Constraint::Percentage(40), // Analysis summary
        ])
        .split(area);

    render_route_list(f, chunks[0], app);
    render_analysis_summary(f, chunks[1], app);
}

/// Render the current route list
fn render_route_list(f: &mut Frame, area: Rect, _app: &App) {
    let state = get_multihop_screen_state();

    let list_style = if matches!(state.input_focus, MultiHopInputFocus::RouteList) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(format!("Current Route ({} hops)", state.route.len()))
        .borders(Borders::ALL)
        .style(list_style);

    if state.route.is_empty() {
        let empty_text =
            Paragraph::new("No hops added yet.\nUse the Route Builder to add swap operations.")
                .block(block)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty_text, area);
        return;
    }

    let items: Vec<ListItem> = state
        .route
        .iter()
        .enumerate()
        .map(|(i, hop)| {
            let content = vec![
                Line::from(vec![
                    Span::styled(format!("{}. ", i + 1), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{} → {}", hop.from_asset, hop.to_asset),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("   Pool: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&hop.pool_name, Style::default().fg(Color::Blue)),
                ]),
                Line::from(vec![
                    Span::styled("   In: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&hop.amount_in, Style::default().fg(Color::Green)),
                    Span::styled(" → Out: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&hop.estimated_amount_out, Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::styled("   Impact: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:.2}%", hop.price_impact),
                        Style::default().fg(Color::Red),
                    ),
                    Span::styled(", Fee: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&hop.fee_amount, Style::default().fg(Color::Yellow)),
                ]),
            ];
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("→ ");

    f.render_stateful_widget(list, area, &mut state.route_list_state);
}

/// Render the analysis summary panel
fn render_analysis_summary(f: &mut Frame, area: Rect, app: &App) {
    let state = get_multihop_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Analysis details
            Constraint::Length(3), // Remove hop button
            Constraint::Length(3), // Execute button
        ])
        .split(area);

    // Analysis details
    let analysis_text = if state.route.is_empty() {
        Text::from("Add hops to see route analysis")
    } else {
        let efficiency_color = if state.route_analysis.route_efficiency >= 80.0 {
            Color::Green
        } else if state.route_analysis.route_efficiency >= 60.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        Text::from(vec![
            Line::from(vec![
                Span::styled("Initial Amount: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!(
                        "{} {}",
                        state.route_analysis.initial_amount,
                        state
                            .route
                            .first()
                            .map(|h| &h.from_asset)
                            .unwrap_or(&"".to_string())
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::styled("Final Estimated: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!(
                        "{} {}",
                        state.route_analysis.final_estimated_amount,
                        state
                            .route
                            .last()
                            .map(|h| &h.to_asset)
                            .unwrap_or(&"".to_string())
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Total Price Impact: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{:.2}%", state.route_analysis.total_price_impact),
                    Style::default().fg(Color::Red),
                ),
            ]),
            Line::from(vec![
                Span::styled("Total Fees: ", Style::default().fg(Color::White)),
                Span::styled(
                    &state.route_analysis.total_fees,
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::styled("Estimated Time: ", Style::default().fg(Color::White)),
                Span::styled(
                    &state.route_analysis.estimated_execution_time,
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Route Efficiency: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{:.1}/100", state.route_analysis.route_efficiency),
                    Style::default().fg(efficiency_color),
                ),
            ]),
        ])
    };

    let analysis = Paragraph::new(analysis_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Route Analysis"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(analysis, chunks[0]);

    // Remove hop button
    let remove_style = if matches!(state.input_focus, MultiHopInputFocus::RemoveHop) {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else if !state.route.is_empty() && state.route_list_state.selected().is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let remove_text = if !state.route.is_empty() && state.route_list_state.selected().is_some() {
        "Press Enter to Remove Selected Hop"
    } else {
        "Select a hop to remove"
    };
    let remove_button = Paragraph::new(remove_text)
        .block(Block::default().borders(Borders::ALL).title("Remove Hop"))
        .style(remove_style)
        .alignment(Alignment::Center);
    f.render_widget(remove_button, chunks[1]);

    // Execute button
    let execute_style = if matches!(state.input_focus, MultiHopInputFocus::Execute) {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if state.validate_route() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let loading_style = matches!(app.state.loading_state, LoadingState::Loading { .. });
    let execute_text = if loading_style {
        "Executing Multi-Hop Swap..."
    } else if state.validate_route() {
        "Press Enter to Execute Route"
    } else {
        "Add hops to execute route"
    };

    let execute_button = Paragraph::new(execute_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Execute Multi-Hop Swap"),
        )
        .style(execute_style)
        .alignment(Alignment::Center);
    f.render_widget(execute_button, chunks[2]);
}

/// Handle input for the multi-hop screen
pub fn handle_multihop_screen_input(input: InputRequest) -> bool {
    let state = get_multihop_screen_state();
    state.handle_input(input)
}

/// Handle navigation for the multi-hop screen
pub fn handle_multihop_screen_navigation(next: bool) {
    let state = get_multihop_screen_state();
    if next {
        state.next_focus();
    } else {
        state.previous_focus();
    }
}

/// Handle Enter key for the multi-hop screen actions
pub fn handle_multihop_screen_action() -> Option<String> {
    let state = get_multihop_screen_state();

    match state.input_focus {
        MultiHopInputFocus::AddHop => {
            if state.validate_current_hop() {
                state.add_hop();
                Some("Hop added to route".to_string())
            } else {
                Some("Please fill all required fields".to_string())
            }
        }
        MultiHopInputFocus::RemoveHop => {
            if !state.route.is_empty() && state.route_list_state.selected().is_some() {
                state.remove_selected_hop();
                Some("Hop removed from route".to_string())
            } else {
                Some("Please select a hop to remove".to_string())
            }
        }
        MultiHopInputFocus::Execute => {
            if state.validate_route() {
                state.show_confirmation_modal();
                Some("Review the multi-hop swap details".to_string())
            } else {
                Some("Please add at least one hop to execute".to_string())
            }
        }
        _ => None,
    }
}

/// Execute the multi-hop swap with confirmation
pub fn execute_multihop_swap_with_confirmation() -> Option<Vec<SwapOperation>> {
    let state = get_multihop_screen_state();
    if state.validate_route() {
        let operations = state.get_swap_operations();
        state.hide_confirmation_modal();
        Some(operations)
    } else {
        None
    }
}

/// Handle confirmation response for multi-hop execution
pub fn handle_multihop_confirmation_response(confirmed: bool) -> bool {
    let state = get_multihop_screen_state();
    if confirmed {
        // Execute the multi-hop swap
        state.hide_confirmation_modal();
        true
    } else {
        state.hide_confirmation_modal();
        false
    }
}

/// Reset the multi-hop form
pub fn reset_multihop_form() {
    let state = get_multihop_screen_state();
    state.clear_route();
    state.from_token_dropdown.clear_selection();
    state.to_token_dropdown.clear_selection();
    state.amount_input.clear();
    state.pool_dropdown.clear_selection();
    state.input_focus = MultiHopInputFocus::FromToken;
    state.hide_confirmation_modal();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multihop_screen_state_navigation() {
        let mut state = MultiHopScreenState::default();
        assert_eq!(state.input_focus, MultiHopInputFocus::FromToken);

        state.next_focus();
        assert_eq!(state.input_focus, MultiHopInputFocus::ToToken);

        state.next_focus();
        assert_eq!(state.input_focus, MultiHopInputFocus::Amount);

        state.previous_focus();
        assert_eq!(state.input_focus, MultiHopInputFocus::ToToken);
    }

    #[test]
    fn test_route_management() {
        let mut state = MultiHopScreenState::default();

        // Add a hop
        state.from_token_dropdown = state
            .from_token_dropdown
            .add_option(DropdownOption::new("USDC".to_string(), "USDC".to_string()));
        state
            .from_token_dropdown
            .select_by_value(&"USDC".to_string());
        state.to_token_dropdown = state
            .to_token_dropdown
            .add_option(DropdownOption::new("ATOM".to_string(), "ATOM".to_string()));
        state.to_token_dropdown.select_by_value(&"ATOM".to_string());
        state.amount_input.set_value("100");
        state.pool_dropdown = state
            .pool_dropdown
            .add_option(DropdownOption::new("Pool 1".to_string(), "1".to_string()));
        state.pool_dropdown.select_by_value(&"1".to_string());

        assert!(state.validate_current_hop());
        state.add_hop();
        assert_eq!(state.route.len(), 1);

        // Remove the hop
        state.route_list_state.select(Some(0));
        state.remove_selected_hop();
        assert_eq!(state.route.len(), 0);
    }

    #[test]
    fn test_route_analysis_calculation() {
        let mut state = MultiHopScreenState::default();
        assert_eq!(state.route_analysis.total_hops, 0);

        // Add a test hop
        let hop = SwapHop {
            from_asset: "USDC".to_string(),
            to_asset: "ATOM".to_string(),
            amount_in: "100".to_string(),
            estimated_amount_out: "95".to_string(),
            price_impact: 1.5,
            fee_amount: "0.3".to_string(),
            ..Default::default()
        };

        state.route.push(hop);
        state.update_route_analysis();

        assert_eq!(state.route_analysis.total_hops, 1);
        assert_eq!(state.route_analysis.initial_amount, "100");
        assert_eq!(state.route_analysis.final_estimated_amount, "95");
        assert_eq!(state.route_analysis.total_price_impact, 1.5);
    }
}
