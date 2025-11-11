//! Data Display Components
//!
//! This module contains components for displaying data in tables, cards, and progress bars.
//! These components are used across different screens to present information consistently.

use crate::tui_dex::app::{AppState, LoadingState, TransactionInfo, TransactionStatus};
use mantra_dex_std::pool_manager::PoolInfoResponse;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
};

/// Render a balance table showing user token balances
pub fn render_balance_table(f: &mut Frame, app_state: &AppState, area: Rect) {
    let balances: Vec<(&String, &String)> = app_state.balances.iter().collect();

    if balances.is_empty() {
        let empty_msg =
            Paragraph::new("No token balances available\nConnect wallet to view balances")
                .style(Style::default().fg(Color::Gray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue))
                        .title("Token Balances"),
                )
                .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Token").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Balance").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = balances
        .iter()
        .map(|(token, balance)| {
            let status = if *balance == "0" || balance.is_empty() {
                Cell::from("Empty").style(Style::default().fg(Color::Red))
            } else {
                Cell::from("Available").style(Style::default().fg(Color::Green))
            };

            Row::new(vec![
                Cell::from(token.as_str()),
                Cell::from(format_balance(balance)),
                status,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Token Balances"),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

/// Render a pool info card showing detailed pool information
pub fn render_pool_info_card(f: &mut Frame, pool_info: &PoolInfoResponse, area: Rect) {
    let pool_assets = pool_info
        .pool_info
        .assets
        .iter()
        .map(|asset| {
            let amount = asset.amount.to_string();
            let denom = &asset.denom;
            format!("{}: {}", denom, format_large_number(&amount))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let pool_type = determine_pool_type(&pool_info.pool_info.pool_type);
    let pool_status = if pool_info.pool_info.status.deposits_enabled
        && pool_info.pool_info.status.withdrawals_enabled
        && pool_info.pool_info.status.swaps_enabled
    {
        ("Available", Color::Green)
    } else {
        ("Disabled", Color::Red)
    };

    let total_share = format_large_number(&pool_info.total_share.amount.to_string());

    let pool_details = format!(
        "Pool ID: {}\n\nAssets:\n{}\n\nTotal Shares: {}\nPool Type: {}\nStatus: {}\n\nFeatures:\n• Deposits: {}\n• Withdrawals: {}\n• Swaps: {}",
        pool_info.pool_info.pool_identifier,
        pool_assets,
        total_share,
        pool_type,
        pool_status.0,
        if pool_info.pool_info.status.deposits_enabled { "✓" } else { "✗" },
        if pool_info.pool_info.status.withdrawals_enabled { "✓" } else { "✗" },
        if pool_info.pool_info.status.swaps_enabled { "✓" } else { "✗" }
    );

    let card = Paragraph::new(pool_details)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(pool_status.1))
                .title(format!(
                    "Pool {} Details",
                    pool_info.pool_info.pool_identifier
                )),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(card, area);
}

/// Render a transaction table showing recent transactions
pub fn render_transaction_table(f: &mut Frame, transactions: &[TransactionInfo], area: Rect) {
    if transactions.is_empty() {
        let empty_msg =
            Paragraph::new("No recent transactions\nExecute operations to see transaction history")
                .style(Style::default().fg(Color::Gray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue))
                        .title("Recent Transactions"),
                )
                .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Hash").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Time").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Gas").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = transactions
        .iter()
        .take(10) // Show only the 10 most recent transactions
        .map(|tx| {
            let hash_display = if tx.hash.len() > 12 {
                format!("{}...{}", &tx.hash[..6], &tx.hash[tx.hash.len() - 6..])
            } else {
                tx.hash.clone()
            };

            let (status_text, status_color) = match tx.status {
                TransactionStatus::Pending => ("Pending", Color::Yellow),
                TransactionStatus::Success => ("Success", Color::Green),
                TransactionStatus::Failed => ("Failed", Color::Red),
                TransactionStatus::Unknown => ("Unknown", Color::Gray),
            };

            let time_display = tx.timestamp.format("%H:%M:%S").to_string();

            let gas_display = match (tx.gas_used, tx.gas_wanted) {
                (Some(used), Some(wanted)) => format!("{}/{}", used, wanted),
                (Some(used), None) => used.to_string(),
                _ => "N/A".to_string(),
            };

            Row::new(vec![
                Cell::from(hash_display),
                Cell::from(tx.operation_type.clone()),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(time_display),
                Cell::from(gas_display),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Recent Transactions"),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

/// Render a progress bar for loading states
pub fn render_progress_bar(f: &mut Frame, loading_state: &LoadingState, area: Rect) {
    match loading_state {
        LoadingState::Idle => {
            // Don't render anything when idle
        }
        LoadingState::Loading { message, .. } => {
            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title("Loading"),
                )
                .gauge_style(Style::default().fg(Color::Yellow))
                .percent(50) // Indeterminate progress
                .label(message.clone());
            f.render_widget(gauge, area);
        }
        LoadingState::Success { message, .. } => {
            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green))
                        .title("Success"),
                )
                .gauge_style(Style::default().fg(Color::Green))
                .percent(100)
                .label(message.clone());
            f.render_widget(gauge, area);
        }
        LoadingState::Error { message, .. } => {
            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .title("Error"),
                )
                .gauge_style(Style::default().fg(Color::Red))
                .percent(0)
                .label(message.clone());
            f.render_widget(gauge, area);
        }
    }
}

/// Render a progress bar with specific percentage for determinate operations
pub fn render_progress_bar_with_percent(
    f: &mut Frame,
    title: &str,
    message: &str,
    percent: u16,
    area: Rect,
) {
    let color = if percent == 100 {
        Color::Green
    } else {
        Color::Blue
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
                .title(title),
        )
        .gauge_style(Style::default().fg(color))
        .percent(percent)
        .label(format!("{} ({}%)", message, percent));

    f.render_widget(gauge, area);
}

/// Format balance for display (truncate very large numbers)
fn format_balance(balance: &str) -> String {
    if let Ok(amount) = balance.parse::<u128>() {
        format_large_number(&amount.to_string())
    } else {
        balance.to_string()
    }
}

/// Format large numbers with appropriate suffixes (K, M, B, T)
pub fn format_large_number(number_str: &str) -> String {
    if let Ok(number) = number_str.parse::<u128>() {
        if number >= 1_000_000_000_000 {
            format!("{:.2}T", number as f64 / 1_000_000_000_000.0)
        } else if number >= 1_000_000_000 {
            format!("{:.2}B", number as f64 / 1_000_000_000.0)
        } else if number >= 1_000_000 {
            format!("{:.2}M", number as f64 / 1_000_000.0)
        } else if number >= 1_000 {
            format!("{:.2}K", number as f64 / 1_000.0)
        } else {
            number.to_string()
        }
    } else {
        number_str.to_string()
    }
}

/// Determine pool type from the pool type enum
fn determine_pool_type(pool_type: &mantra_dex_std::pool_manager::PoolType) -> &'static str {
    match pool_type {
        mantra_dex_std::pool_manager::PoolType::ConstantProduct => "Constant Product",
        mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "Stable Swap",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_large_number() {
        assert_eq!(format_large_number("1000"), "1.00K");
        assert_eq!(format_large_number("1000000"), "1.00M");
        assert_eq!(format_large_number("1000000000"), "1.00B");
        assert_eq!(format_large_number("1000000000000"), "1.00T");
        assert_eq!(format_large_number("999"), "999");
        assert_eq!(format_large_number("invalid"), "invalid");
    }

    #[test]
    fn test_format_balance() {
        assert_eq!(format_balance("1000"), "1.00K");
        assert_eq!(format_balance("0"), "0");
        assert_eq!(format_balance("invalid"), "invalid");
    }

    #[test]
    fn test_determine_pool_type() {
        let constant_product = mantra_dex_std::pool_manager::PoolType::ConstantProduct;
        assert_eq!(determine_pool_type(&constant_product), "Constant Product");
    }
}
