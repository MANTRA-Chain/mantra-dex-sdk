//! Pools Screen Implementation
//!
//! This module provides the pools view for the MANTRA DEX SDK TUI,
//! displaying pool listings, details, search functionality, and status indicators.

use crate::tui_dex::{
    app::{App, LoadingState, PoolCacheEntry},
    components::{
        header::render_header, navigation::render_navigation, status_bar::render_status_bar,
    },
};
use mantra_dex_std::pool_manager::PoolInfoResponse;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::collections::HashMap;

/// Pool sorting criteria
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSortBy {
    PoolId,
    AssetPair,
    Tvl,
    Status,
}

/// Pool filter criteria
#[derive(Debug, Clone)]
pub struct PoolFilter {
    pub search_term: String,
    pub show_enabled_only: bool,
    pub asset_filter: Option<String>,
}

impl Default for PoolFilter {
    fn default() -> Self {
        Self {
            search_term: String::new(),
            show_enabled_only: false,
            asset_filter: None,
        }
    }
}

/// Pool display data for the table
#[derive(Debug, Clone)]
pub struct PoolDisplayData {
    pub pool_id: String,
    pub asset_pair: String,
    pub tvl: String,
    pub apy: String,
    pub status: PoolDisplayStatus,
    pub pool_info: PoolInfoResponse,
}

/// Pool status for display purposes
#[derive(Debug, Clone, PartialEq)]
pub enum PoolDisplayStatus {
    Available,
    Disabled,
    PartiallyDisabled,
}

impl PoolDisplayStatus {
    pub fn color(&self) -> Color {
        match self {
            PoolDisplayStatus::Available => Color::Green,
            PoolDisplayStatus::Disabled => Color::Red,
            PoolDisplayStatus::PartiallyDisabled => Color::Yellow,
        }
    }

    pub fn display_text(&self) -> &'static str {
        match self {
            PoolDisplayStatus::Available => "Available",
            PoolDisplayStatus::Disabled => "Disabled",
            PoolDisplayStatus::PartiallyDisabled => "Partial",
        }
    }
}

/// Render the complete pools screen
pub fn render_pools(f: &mut Frame, app: &App) {
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

    // Render pools content
    render_pools_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);
}

/// Render the main pools content area
fn render_pools_content(f: &mut Frame, area: Rect, app: &App) {
    // Create horizontal layout: pools list | pool details
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Create vertical layout for left side: search + filters + pool list
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Length(5), // Filter controls
            Constraint::Min(0),    // Pool list table
        ])
        .split(main_chunks[0]);

    // Render components
    render_pool_search(f, left_chunks[0], app);
    render_pool_filters(f, left_chunks[1], app);
    render_pool_list_table(f, left_chunks[2], app);
    render_pool_details_panel(f, main_chunks[1], app);
}

/// Render the pool search bar
fn render_pool_search(f: &mut Frame, area: Rect, _app: &App) {
    let search_block = Block::default()
        .title("Search Pools")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // For now, show a placeholder. In a full implementation, this would be an input field
    let search_text = "Type to search pools...";
    let search_content = Paragraph::new(search_text)
        .style(Style::default().fg(Color::Gray))
        .block(search_block);

    f.render_widget(search_content, area);
}

/// Render pool filter controls
fn render_pool_filters(f: &mut Frame, area: Rect, _app: &App) {
    let filter_block = Block::default()
        .title("Filters")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let filter_content = vec![
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::White)),
            Span::styled("All Pools", Style::default().fg(Color::Cyan)),
            Span::styled(" [A]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::White)),
            Span::styled("Enabled Only", Style::default().fg(Color::Green)),
            Span::styled(" [E]", Style::default().fg(Color::Gray)),
        ]),
    ];

    let filters = Paragraph::new(Text::from(filter_content))
        .block(filter_block)
        .wrap(Wrap { trim: true });

    f.render_widget(filters, area);
}

/// Render the pool list table
fn render_pool_list_table(f: &mut Frame, area: Rect, app: &App) {
    // Prepare pool data
    let pool_data = prepare_pool_display_data(&app.state.pool_cache);

    if pool_data.is_empty() {
        render_empty_pool_list(f, area, app);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Pool ID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Asset Pair").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("TVL").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = pool_data
        .iter()
        .enumerate()
        .map(|(index, pool)| {
            let style = if app.state.selected_pool_id == Some(pool.pool_id.parse().unwrap_or(0)) {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if index % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(Color::DarkGray)
            };

            Row::new(vec![
                Cell::from(pool.pool_id.clone()),
                Cell::from(pool.asset_pair.clone()),
                Cell::from(pool.tvl.clone()),
                Cell::from(pool.status.display_text()).style(
                    Style::default()
                        .fg(pool.status.color())
                        .add_modifier(Modifier::BOLD),
                ),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),      // Pool ID
            Constraint::Percentage(40), // Asset Pair
            Constraint::Percentage(25), // TVL
            Constraint::Length(12),     // Status
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(format!("Pools ({} total)", pool_data.len())),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

/// Render empty pool list message
fn render_empty_pool_list(f: &mut Frame, area: Rect, app: &App) {
    let message = match app.state.loading_state {
        LoadingState::Loading { .. } => "Loading pools...",
        LoadingState::Error { .. } => "Error loading pools. Check connection and try again.",
        _ => "No pools available\nCheck network connection or try refreshing.",
    };

    let color = match app.state.loading_state {
        LoadingState::Loading { .. } => Color::Yellow,
        LoadingState::Error { .. } => Color::Red,
        _ => Color::Gray,
    };

    let empty_msg = Paragraph::new(message)
        .style(Style::default().fg(color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Pools"),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(empty_msg, area);
}

/// Render the pool details panel
fn render_pool_details_panel(f: &mut Frame, area: Rect, app: &App) {
    if let Some(pool_id) = app.state.selected_pool_id {
        if let Some(pool_cache_entry) = app.state.pool_cache.get(&pool_id.to_string()) {
            render_selected_pool_details(f, area, &pool_cache_entry.pool_info);
        } else {
            render_no_pool_details(f, area, "Pool details not available");
        }
    } else {
        render_no_pool_details(f, area, "Select a pool to view details");
    }
}

/// Render details for the selected pool
fn render_selected_pool_details(f: &mut Frame, area: Rect, pool_info: &PoolInfoResponse) {
    // Split details panel into sections
    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Basic info
            Constraint::Length(10), // Assets composition
            Constraint::Min(0),     // Fee structure and features
        ])
        .split(area);

    render_pool_basic_info(f, detail_chunks[0], pool_info);
    render_pool_composition(f, detail_chunks[1], pool_info);
    render_pool_features(f, detail_chunks[2], pool_info);
}

/// Render basic pool information
fn render_pool_basic_info(f: &mut Frame, area: Rect, pool_info: &PoolInfoResponse) {
    let pool_type = determine_pool_type(&pool_info.pool_info.pool_type);
    let total_shares = format_large_number(&pool_info.total_share.amount.to_string());

    let content = vec![
        Line::from(vec![
            Span::styled("Pool ID: ", Style::default().fg(Color::White)),
            Span::styled(
                pool_info.pool_info.pool_identifier.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::White)),
            Span::styled(pool_type, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Total Shares: ", Style::default().fg(Color::White)),
            Span::styled(total_shares, Style::default().fg(Color::Green)),
        ]),
    ];

    let block = Block::default()
        .title("Pool Information")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render pool asset composition
fn render_pool_composition(f: &mut Frame, area: Rect, pool_info: &PoolInfoResponse) {
    let assets: Vec<String> = pool_info
        .pool_info
        .assets
        .iter()
        .map(|asset| {
            let amount = format_large_number(&asset.amount.to_string());
            format!("• {}: {}", asset.denom, amount)
        })
        .collect();

    let content = if assets.is_empty() {
        vec![Line::from("No assets available")]
    } else {
        assets.into_iter().map(Line::from).collect()
    };

    let block = Block::default()
        .title("Asset Composition")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render pool features and fee structure
fn render_pool_features(f: &mut Frame, area: Rect, pool_info: &PoolInfoResponse) {
    let status = &pool_info.pool_info.status;

    let features = vec![
        Line::from(vec![Span::styled(
            "Operations:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• Deposits: ", Style::default().fg(Color::White)),
            Span::styled(
                if status.deposits_enabled {
                    "Enabled ✓"
                } else {
                    "Disabled ✗"
                },
                Style::default().fg(if status.deposits_enabled {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("• Withdrawals: ", Style::default().fg(Color::White)),
            Span::styled(
                if status.withdrawals_enabled {
                    "Enabled ✓"
                } else {
                    "Disabled ✗"
                },
                Style::default().fg(if status.withdrawals_enabled {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("• Swaps: ", Style::default().fg(Color::White)),
            Span::styled(
                if status.swaps_enabled {
                    "Enabled ✓"
                } else {
                    "Disabled ✗"
                },
                Style::default().fg(if status.swaps_enabled {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pool Features:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• Fee Strategy: ", Style::default().fg(Color::White)),
            Span::styled("Standard", Style::default().fg(Color::Cyan)), // Placeholder
        ]),
    ];

    let block = Block::default()
        .title("Features & Configuration")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let paragraph = Paragraph::new(Text::from(features))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render message when no pool is selected
fn render_no_pool_details(f: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .title("Pool Details")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray))
        .padding(Padding::uniform(1));

    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Gray))
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Prepare pool data for display in the table
fn prepare_pool_display_data(pool_cache: &HashMap<String, PoolCacheEntry>) -> Vec<PoolDisplayData> {
    let mut pools: Vec<PoolDisplayData> = pool_cache
        .values()
        .map(|cache_entry| {
            let pool_info = &cache_entry.pool_info;
            let asset_pair = create_asset_pair_string(&pool_info.pool_info.assets);
            let tvl = calculate_pool_tvl(&pool_info.pool_info.assets);
            let status = determine_pool_status(&pool_info.pool_info.status);

            PoolDisplayData {
                pool_id: pool_info.pool_info.pool_identifier.to_string(),
                asset_pair,
                tvl,
                apy: "N/A".to_string(), // APY calculation would require historical data
                status,
                pool_info: pool_info.clone(),
            }
        })
        .collect();

    // Sort by pool ID by default
    pools.sort_by(|a, b| {
        a.pool_id
            .parse::<u64>()
            .unwrap_or(0)
            .cmp(&b.pool_id.parse::<u64>().unwrap_or(0))
    });

    pools
}

/// Create a readable asset pair string
fn create_asset_pair_string(assets: &[cosmwasm_std::Coin]) -> String {
    if assets.is_empty() {
        return "No assets".to_string();
    }

    let asset_names: Vec<String> = assets
        .iter()
        .map(|asset| {
            // Extract token name from denom (e.g., "factory/..." -> shortened name)
            let denom = &asset.denom;
            if denom.len() > 20 {
                if let Some(last_part) = denom.split('/').last() {
                    last_part.to_string()
                } else {
                    format!("{}...", &denom[..10])
                }
            } else {
                denom.to_string()
            }
        })
        .collect();

    asset_names.join(" / ")
}

/// Calculate total value locked (simplified calculation)
fn calculate_pool_tvl(assets: &[cosmwasm_std::Coin]) -> String {
    if assets.is_empty() {
        return "0".to_string();
    }

    // Simplified TVL calculation - in reality, this would require price data
    let total_assets: u128 = assets.iter().map(|asset| asset.amount.u128()).sum();

    if total_assets == 0 {
        "0".to_string()
    } else {
        format_large_number(&total_assets.to_string())
    }
}

/// Determine pool status based on enabled operations
fn determine_pool_status(status: &mantra_dex_std::pool_manager::PoolStatus) -> PoolDisplayStatus {
    let enabled_count = [
        status.deposits_enabled,
        status.withdrawals_enabled,
        status.swaps_enabled,
    ]
    .iter()
    .filter(|&&enabled| enabled)
    .count();

    match enabled_count {
        3 => PoolDisplayStatus::Available,
        0 => PoolDisplayStatus::Disabled,
        _ => PoolDisplayStatus::PartiallyDisabled,
    }
}

/// Format large numbers with appropriate suffixes
fn format_large_number(number_str: &str) -> String {
    if let Ok(number) = number_str.parse::<u128>() {
        if number >= 1_000_000_000_000 {
            format!("{:.1}T", number as f64 / 1_000_000_000_000.0)
        } else if number >= 1_000_000_000 {
            format!("{:.1}B", number as f64 / 1_000_000_000.0)
        } else if number >= 1_000_000 {
            format!("{:.1}M", number as f64 / 1_000_000.0)
        } else if number >= 1_000 {
            format!("{:.1}K", number as f64 / 1_000.0)
        } else {
            number.to_string()
        }
    } else {
        number_str.to_string()
    }
}

/// Determine pool type display name
fn determine_pool_type(pool_type: &mantra_dex_std::pool_manager::PoolType) -> &'static str {
    match pool_type {
        mantra_dex_std::pool_manager::PoolType::ConstantProduct => "Constant Product",
        mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "Stable Swap",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::Coin;

    #[test]
    fn test_format_large_number() {
        assert_eq!(format_large_number("1500000000000"), "1.5T");
        assert_eq!(format_large_number("1500000000"), "1.5B");
        assert_eq!(format_large_number("1500000"), "1.5M");
        assert_eq!(format_large_number("1500"), "1.5K");
        assert_eq!(format_large_number("150"), "150");
    }

    #[test]
    fn test_create_asset_pair_string() {
        let assets = vec![
            Coin::new(1000000u128, "uom"),
            Coin::new(2000000u128, "factory/contract/token"),
        ];
        let result = create_asset_pair_string(&assets);
        assert!(result.contains("uom"));
        assert!(result.contains("token"));
        assert!(result.contains(" / "));
    }

    #[test]
    fn test_create_asset_pair_string_empty() {
        let assets = vec![];
        assert_eq!(create_asset_pair_string(&assets), "No assets");
    }

    #[test]
    fn test_determine_pool_status() {
        let fully_enabled = mantra_dex_std::pool_manager::PoolStatus {
            deposits_enabled: true,
            withdrawals_enabled: true,
            swaps_enabled: true,
        };
        assert_eq!(
            determine_pool_status(&fully_enabled),
            PoolDisplayStatus::Available
        );

        let fully_disabled = mantra_dex_std::pool_manager::PoolStatus {
            deposits_enabled: false,
            withdrawals_enabled: false,
            swaps_enabled: false,
        };
        assert_eq!(
            determine_pool_status(&fully_disabled),
            PoolDisplayStatus::Disabled
        );

        let partially_enabled = mantra_dex_std::pool_manager::PoolStatus {
            deposits_enabled: true,
            withdrawals_enabled: false,
            swaps_enabled: true,
        };
        assert_eq!(
            determine_pool_status(&partially_enabled),
            PoolDisplayStatus::PartiallyDisabled
        );
    }
}
