//! Rewards Screen Implementation
//!
//! This module provides the rewards view for the MANTRA DEX SDK TUI,
//! displaying claimable rewards, claim interface, rewards history, and epoch timeline.

use crate::tui_dex::{
    app::{App, LoadingState},
    components::{
        header::render_header, navigation::render_navigation, status_bar::render_status_bar,
    },
};
use cosmwasm_std::Uint128;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

/// Current rewards screen mode
#[derive(Debug, Clone, PartialEq)]
pub enum RewardsMode {
    Dashboard,
    Claim,
    History,
    EpochTimeline,
}

/// Rewards screen state
#[derive(Debug, Clone)]
pub struct RewardsState {
    pub mode: RewardsMode,
    pub selected_epoch: Option<u64>,
    pub claim_input: String,
    pub show_claim_confirmation: bool,
    pub table_selected: usize,
    pub history_scroll: usize,
}

impl Default for RewardsState {
    fn default() -> Self {
        Self {
            mode: RewardsMode::Dashboard,
            selected_epoch: None,
            claim_input: String::new(),
            show_claim_confirmation: false,
            table_selected: 0,
            history_scroll: 0,
        }
    }
}

/// Render the complete rewards screen
pub fn render_rewards(f: &mut Frame, app: &App) {
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

    // Render rewards content based on current mode
    render_rewards_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);
}

/// Render the main rewards content area
fn render_rewards_content(f: &mut Frame, area: Rect, app: &App) {
    // Create a 2x2 grid layout for the rewards screen
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main_chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // Render components
    render_rewards_dashboard(f, top_chunks[0], app);
    render_claim_interface(f, top_chunks[1], app);
    render_rewards_history(f, bottom_chunks[0], app);
    render_epoch_timeline(f, bottom_chunks[1], app);
}

/// Render the rewards dashboard panel
fn render_rewards_dashboard(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Rewards Dashboard")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::uniform(1));

    let content = if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
        vec![Line::from(vec![Span::styled(
            "Loading rewards data...",
            Style::default().fg(Color::Yellow),
        )])]
    } else {
        let total_claimable = calculate_total_claimable_rewards(&app.state.claimable_rewards);
        let num_pools_with_rewards = app.state.claimable_rewards.len();
        let current_epoch = app.state.current_epoch.unwrap_or(0);

        vec![
            Line::from(vec![Span::styled(
                "Total Claimable Rewards",
                Style::default().fg(Color::White),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Amount: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{} tokens", format_amount(total_claimable)),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Pools with Rewards: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}", num_pools_with_rewards),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Current Epoch: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}", current_epoch),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from("Rewards by Pool:"),
            Line::from(""),
        ]
        .into_iter()
        .chain(render_rewards_by_pool(&app.state.claimable_rewards))
        .collect()
    };

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render the claim interface panel
fn render_claim_interface(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Claim Interface")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let current_epoch = app.state.current_epoch.unwrap_or(0);

    let content = vec![
        Line::from(vec![Span::styled(
            "Claim Options:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[1] ", Style::default().fg(Color::Cyan)),
            Span::styled("Claim All Rewards", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[2] ", Style::default().fg(Color::Cyan)),
            Span::styled("Claim Until Epoch", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Current Epoch: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", current_epoch),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Enter epoch number:",
            Style::default().fg(Color::White),
        )]),
        Line::from(vec![
            Span::styled("(1 to ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", current_epoch),
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(")", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from("Instructions:"),
        Line::from("- Press '1' to claim all"),
        Line::from("- Press '2' to claim until epoch"),
        Line::from("- Use number keys to enter epoch"),
        Line::from("- Press Enter to confirm"),
        Line::from("- Press Esc to cancel"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render the rewards history panel
fn render_rewards_history(f: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .title("Rewards History")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    // Mock rewards history data - in a real implementation, this would come from the app state
    let history_items = generate_mock_rewards_history();

    let content = if history_items.is_empty() {
        vec![Line::from(vec![Span::styled(
            "No rewards history available",
            Style::default().fg(Color::Gray),
        )])]
    } else {
        let mut lines = vec![
            Line::from(vec![Span::styled(
                "Recent Rewards Claims:",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
        ];

        for (i, item) in history_items.iter().enumerate() {
            if i >= 8 {
                break;
            } // Limit display to fit area

            lines.push(Line::from(vec![
                Span::styled(&item.date, Style::default().fg(Color::Cyan)),
                Span::styled(" | ", Style::default().fg(Color::Gray)),
                Span::styled(&item.amount, Style::default().fg(Color::Green)),
                Span::styled(" | ", Style::default().fg(Color::Gray)),
                Span::styled(&item.epoch, Style::default().fg(Color::Magenta)),
            ]));
        }

        lines
    };

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render the epoch timeline panel
fn render_epoch_timeline(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Epoch Timeline")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let current_epoch = app.state.current_epoch.unwrap_or(0);

    // Create a visual timeline of recent epochs
    let mut content = vec![
        Line::from(vec![Span::styled(
            "Epoch Timeline:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
    ];

    // Show last 10 epochs with current epoch highlighted
    let start_epoch = if current_epoch >= 10 {
        current_epoch - 9
    } else {
        1
    };

    for epoch in start_epoch..=current_epoch {
        let is_current = epoch == current_epoch;
        let has_rewards = !app.state.claimable_rewards.is_empty(); // Simplified check

        let epoch_line = if is_current {
            Line::from(vec![
                Span::styled("►", Style::default().fg(Color::Green)),
                Span::styled(
                    format!(" Epoch {}", epoch),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" (Current)", Style::default().fg(Color::Green)),
            ])
        } else {
            let color = if has_rewards {
                Color::Cyan
            } else {
                Color::Gray
            };
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("Epoch {}", epoch), Style::default().fg(color)),
                if has_rewards {
                    Span::styled(" ✓", Style::default().fg(Color::Green))
                } else {
                    Span::styled("", Style::default())
                },
            ])
        };

        content.push(epoch_line);
    }

    // Add legend
    content.push(Line::from(""));
    content.push(Line::from(vec![Span::styled(
        "Legend:",
        Style::default().fg(Color::White),
    )]));
    content.push(Line::from(vec![
        Span::styled("► ", Style::default().fg(Color::Green)),
        Span::styled("Current Epoch", Style::default().fg(Color::White)),
    ]));
    content.push(Line::from(vec![
        Span::styled("✓ ", Style::default().fg(Color::Green)),
        Span::styled("Has Rewards", Style::default().fg(Color::White)),
    ]));

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Helper function to calculate total claimable rewards
fn calculate_total_claimable_rewards(rewards: &HashMap<String, Uint128>) -> u64 {
    rewards.values().map(|amount| amount.u128() as u64).sum()
}

/// Helper function to render rewards by pool
fn render_rewards_by_pool(rewards: &HashMap<String, Uint128>) -> Vec<Line> {
    if rewards.is_empty() {
        return vec![Line::from(vec![Span::styled(
            "No claimable rewards",
            Style::default().fg(Color::Gray),
        )])];
    }

    let mut lines = Vec::new();
    for (pool_id, amount) in rewards.iter() {
        lines.push(Line::from(vec![
            Span::styled("Pool ", Style::default().fg(Color::White)),
            Span::styled(pool_id, Style::default().fg(Color::Cyan)),
            Span::styled(": ", Style::default().fg(Color::White)),
            Span::styled(
                format_amount(amount.u128() as u64).to_string(),
                Style::default().fg(Color::Green),
            ),
        ]));
    }
    lines
}

/// Helper function to format amounts
fn format_amount(amount: u64) -> String {
    if amount >= 1_000_000 {
        format!("{:.2}M", amount as f64 / 1_000_000.0)
    } else if amount >= 1_000 {
        format!("{:.2}K", amount as f64 / 1_000.0)
    } else {
        amount.to_string()
    }
}

/// Mock rewards history item
#[derive(Debug, Clone)]
struct RewardsHistoryItem {
    date: String,
    amount: String,
    epoch: String,
}

/// Generate mock rewards history for demonstration
fn generate_mock_rewards_history() -> Vec<RewardsHistoryItem> {
    vec![
        RewardsHistoryItem {
            date: "2024-01-15".to_string(),
            amount: "150.5 MANTRA".to_string(),
            epoch: "Epoch 45".to_string(),
        },
        RewardsHistoryItem {
            date: "2024-01-10".to_string(),
            amount: "89.2 MANTRA".to_string(),
            epoch: "Epoch 44".to_string(),
        },
        RewardsHistoryItem {
            date: "2024-01-05".to_string(),
            amount: "234.7 MANTRA".to_string(),
            epoch: "Epoch 43".to_string(),
        },
        RewardsHistoryItem {
            date: "2023-12-30".to_string(),
            amount: "67.8 MANTRA".to_string(),
            epoch: "Epoch 42".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_total_claimable_rewards() {
        let mut rewards = HashMap::new();
        rewards.insert("pool1".to_string(), Uint128::new(1000));
        rewards.insert("pool2".to_string(), Uint128::new(2000));

        assert_eq!(calculate_total_claimable_rewards(&rewards), 3000);
    }

    #[test]
    fn test_calculate_total_claimable_rewards_empty() {
        let rewards = HashMap::new();
        assert_eq!(calculate_total_claimable_rewards(&rewards), 0);
    }

    #[test]
    fn test_format_amount() {
        assert_eq!(format_amount(500), "500");
        assert_eq!(format_amount(1500), "1.50K");
        assert_eq!(format_amount(1500000), "1.50M");
    }

    #[test]
    fn test_rewards_state_default() {
        let state = RewardsState::default();
        assert_eq!(state.mode, RewardsMode::Dashboard);
        assert_eq!(state.selected_epoch, None);
        assert_eq!(state.claim_input, "");
        assert_eq!(state.show_claim_confirmation, false);
    }
}
