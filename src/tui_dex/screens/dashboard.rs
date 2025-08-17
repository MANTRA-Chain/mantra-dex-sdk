//! Dashboard Screen Implementation
//!
//! This module provides the main dashboard view for the MANTRA DEX SDK TUI,
//! displaying portfolio overview, quick stats, recent transactions, and network health.

use crate::tui_dex::{
    app::{App, LoadingState, TransactionStatus},
    components::{
        charts::{render_network_sync_progress, render_transaction_confirmation_progress},
        header::render_header,
        navigation::render_navigation,
        status_bar::render_status_bar,
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

/// Render the complete dashboard screen
pub fn render_dashboard(f: &mut Frame, app: &App) {
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

    // Render dashboard content
    render_dashboard_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);
}

/// Render the main dashboard content area
fn render_dashboard_content(f: &mut Frame, area: Rect, app: &App) {
    // Create a 3-row layout for the dashboard
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30), // Top row: Overview + Quick Stats
            Constraint::Percentage(35), // Middle row: Token Balances + Network Health
            Constraint::Percentage(35), // Bottom row: Recent Transactions
        ])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[0]);

    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[1]);

    // Render components with focus awareness
    render_overview_panel(f, top_chunks[0], app);
    render_quick_stats(f, top_chunks[1], app);
    render_token_balances(f, middle_chunks[0], app);
    render_network_health(f, middle_chunks[1], app);
    render_recent_transactions(f, main_chunks[2], app);

    // Render focus indicators for dashboard elements
    if app.state.navigation_mode == crate::tui_dex::app::NavigationMode::WithinScreen {
        render_dashboard_focus_indicators(f, area, app);
    }
}

/// Render focus indicators for dashboard elements when in content mode
fn render_dashboard_focus_indicators(f: &mut Frame, area: Rect, app: &App) {
    if let Some(focused) = app.state.focus_manager.current_focus() {
        match focused {
            crate::tui_dex::events::FocusableComponent::Button(button_id) => {
                if button_id == "dashboard_refresh" {
                    // Show a focused refresh button overlay
                    let button_area = Rect {
                        x: area.x + 2,
                        y: area.y + 2,
                        width: 20,
                        height: 3,
                    };

                    let button = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title("[ REFRESH ]")
                        .title_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );

                    f.render_widget(button, button_area);
                }
            }
            crate::tui_dex::events::FocusableComponent::Table(table_id) => {
                if table_id == "dashboard_transactions" {
                    // Highlight the transactions area
                    let tx_area = Rect {
                        x: area.x + area.width / 2,
                        y: area.y + area.height * 2 / 5,
                        width: area.width / 2 - 2,
                        height: area.height * 3 / 5 - 2,
                    };

                    let highlight = Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                        .title("[ FOCUSED: TRANSACTIONS ]")
                        .title_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );

                    f.render_widget(highlight, tx_area);
                }
            }
            _ => {}
        }
    }
}

/// Render the portfolio overview panel
fn render_overview_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Portfolio Overview")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::uniform(1));

    // Calculate portfolio metrics
    let total_value = calculate_total_portfolio_value(&app.state.balances);
    let active_positions = count_active_positions(&app.state.pool_cache);
    let recent_activity_count = app.state.recent_transactions.len();

    let content = if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
        vec![Line::from(vec![Span::styled(
            "Loading portfolio data...",
            Style::default().fg(Color::Yellow),
        )])]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Total Portfolio Value: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("${:.2}", total_value),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Active Positions: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}", active_positions),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Recent Activity: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{} transactions", recent_activity_count),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Wallet: ", Style::default().fg(Color::White)),
                Span::styled(
                    app.state
                        .wallet_address
                        .as_ref()
                        .map(|addr| format!("{}...{}", &addr[..8], &addr[addr.len() - 8..]))
                        .unwrap_or_else(|| "Not connected".to_string()),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
        ]
    };

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render quick statistics panel
fn render_quick_stats(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Quick Stats")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let total_pools = app.state.pool_cache.len();
    let network_status = if app.state.network_info.is_syncing {
        ("Syncing", Color::Yellow)
    } else {
        ("Synced", Color::Green)
    };

    let content = vec![
        Line::from(vec![
            Span::styled("Total Pools: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", total_pools),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Network: ", Style::default().fg(Color::White)),
            Span::styled(
                network_status.0,
                Style::default()
                    .fg(network_status.1)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Block Height: ", Style::default().fg(Color::White)),
            Span::styled(
                app.state
                    .block_height
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "Unknown".to_string()),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Current Epoch: ", Style::default().fg(Color::White)),
            Span::styled(
                app.state
                    .current_epoch
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Unknown".to_string()),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Chain ID: ", Style::default().fg(Color::White)),
            Span::styled(
                app.state
                    .network_info
                    .chain_id
                    .as_deref()
                    .unwrap_or("Unknown"),
                Style::default().fg(Color::Gray),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render token balances panel
fn render_token_balances(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Token Balances")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app.state.wallet_address.is_none() {
        let no_wallet_msg = Paragraph::new("No wallet connected")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(no_wallet_msg, inner_area);
        return;
    }

    if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
        let loading_msg = Paragraph::new("Loading balances...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(loading_msg, inner_area);
        return;
    }

    // Get formatted balances using proper decimals
    let formatted_balances = app.get_formatted_balances();

    if formatted_balances.is_empty() {
        let empty_msg = Paragraph::new("No token balances found")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner_area);
        return;
    }

    // Create content lines for balances
    let mut content_lines = Vec::new();

    // Show up to the available lines (leave room for spacing)
    let max_tokens = (inner_area.height as usize).saturating_sub(1);
    let displayed_balances = if formatted_balances.len() > max_tokens {
        &formatted_balances[..max_tokens.saturating_sub(1)] // Leave room for "..." indicator
    } else {
        &formatted_balances[..]
    };

    for (symbol, amount, denom) in displayed_balances {
        // Create a formatted line with symbol and amount
        let line = if symbol == denom {
            // Raw denomination, show truncated version
            let display_denom = if denom.len() > 20 {
                format!("{}...", &denom[..17])
            } else {
                denom.clone()
            };
            Line::from(vec![
                Span::styled(
                    format!("{:<12} ", display_denom),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    amount,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            // Has a symbol, show symbol and amount
            Line::from(vec![
                Span::styled(format!("{:<12} ", symbol), Style::default().fg(Color::Cyan)),
                Span::styled(
                    amount,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        };
        content_lines.push(line);
    }

    // Add "..." indicator if there are more tokens
    if formatted_balances.len() > max_tokens {
        content_lines.push(Line::from(vec![Span::styled(
            format!(
                "... and {} more",
                formatted_balances.len() - displayed_balances.len()
            ),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]));
    }

    let paragraph = Paragraph::new(Text::from(content_lines))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(paragraph, inner_area);
}

/// Render recent transactions with enhanced progress visualization for pending ones
fn render_recent_transactions(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Transactions & Progress")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app.state.recent_transactions.is_empty() {
        let empty_msg = Paragraph::new("No recent transactions")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner_area);
        return;
    }

    // Check if there are pending transactions to show progress bars
    let pending_count = app
        .state
        .recent_transactions
        .iter()
        .filter(|tx| tx.status == TransactionStatus::Pending)
        .count();

    if pending_count > 0 {
        // Split area: progress bars for pending transactions + transaction list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length((pending_count.min(3) * 4) as u16), // Progress bars (max 3)
                Constraint::Length(1),                                 // Spacer
                Constraint::Min(0),                                    // Transaction list
            ])
            .split(inner_area);

        // Render transaction confirmation progress for pending transactions
        render_transaction_confirmation_progress(f, &app.state.recent_transactions, chunks[0]);

        // Render transaction list in the remaining space
        render_transaction_list(f, chunks[2], app);
    } else {
        // No pending transactions, just show the list
        render_transaction_list(f, inner_area, app);
    }
}

/// Render the transaction list portion
fn render_transaction_list(f: &mut Frame, area: Rect, app: &App) {
    // Create list items from recent transactions
    let items: Vec<ListItem> = app
        .state
        .recent_transactions
        .iter()
        .take(8) // Show last 8 transactions to make room for progress bars
        .map(|tx| {
            let status_color = match tx.status {
                TransactionStatus::Success => Color::Green,
                TransactionStatus::Failed => Color::Red,
                TransactionStatus::Pending => Color::Yellow,
                TransactionStatus::Unknown => Color::Gray,
            };

            let status_symbol = match tx.status {
                TransactionStatus::Success => "✓",
                TransactionStatus::Failed => "✗",
                TransactionStatus::Pending => "⏳",
                TransactionStatus::Unknown => "?",
            };

            let hash_short = format!("{}...{}", &tx.hash[..8], &tx.hash[tx.hash.len() - 8..]);
            let time_str = tx.timestamp.format("%H:%M:%S").to_string();

            ListItem::new(Line::from(vec![
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(tx.operation_type.clone(), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(hash_short, Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(time_str, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Recent Transactions")
            .borders(Borders::ALL),
    );
    f.render_widget(list, area);
}

/// Render network health indicators with enhanced progress visualization
fn render_network_health(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Network Health & Progress")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Create layout for enhanced progress bars and info
    let health_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Enhanced sync progress with ETA
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Status info
        ])
        .split(inner_area);

    // Render enhanced network sync progress bar
    render_network_sync_progress(f, &app.state, health_chunks[0]);

    // Network status information
    let last_sync = app
        .state
        .network_info
        .last_sync_time
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "Never".to_string());

    let status_content = vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::White)),
            Span::styled(
                if app.state.network_info.is_syncing {
                    "Syncing"
                } else {
                    "Healthy"
                },
                if app.state.network_info.is_syncing {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Last Sync: ", Style::default().fg(Color::White)),
            Span::styled(last_sync, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("Node: ", Style::default().fg(Color::White)),
            Span::styled(
                app.state
                    .network_info
                    .node_version
                    .as_deref()
                    .unwrap_or("Unknown"),
                Style::default().fg(Color::Blue),
            ),
        ]),
    ];

    let status_paragraph = Paragraph::new(Text::from(status_content)).wrap(Wrap { trim: true });

    f.render_widget(status_paragraph, health_chunks[2]);
}

/// Calculate total portfolio value from balances
fn calculate_total_portfolio_value(balances: &HashMap<String, String>) -> f64 {
    // In a real implementation, you would fetch current prices and calculate
    // the total value. For now, we'll return a mock value.
    if balances.is_empty() {
        0.0
    } else {
        // Mock calculation - sum up balance values
        balances
            .values()
            .filter_map(|balance_str| balance_str.parse::<f64>().ok())
            .sum::<f64>()
            * 0.25 // Mock price multiplier
    }
}

/// Count active liquidity positions
fn count_active_positions(
    pool_cache: &HashMap<String, crate::tui_dex::app::PoolCacheEntry>,
) -> usize {
    // In a real implementation, you would check which pools the user has LP positions in
    // For now, we'll return the number of cached pools as a proxy
    pool_cache.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_total_portfolio_value() {
        let mut balances = HashMap::new();
        balances.insert("ATOM".to_string(), "100.0".to_string());
        balances.insert("USDC".to_string(), "50.0".to_string());

        let total = calculate_total_portfolio_value(&balances);
        assert_eq!(total, 37.5); // (100 + 50) * 0.25
    }

    #[test]
    fn test_calculate_total_portfolio_value_empty() {
        let balances = HashMap::new();
        let total = calculate_total_portfolio_value(&balances);
        assert_eq!(total, 0.0);
    }

    #[test]
    fn test_count_active_positions() {
        let pool_cache = HashMap::new();
        let count = count_active_positions(&pool_cache);
        assert_eq!(count, 0);
    }
}
