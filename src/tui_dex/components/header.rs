//! Header Component
//!
//! This component displays the top header bar with application title,
//! network status, wallet address, and block height information.

use crate::tui_dex::app::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// Render the header component
pub fn render_header(f: &mut Frame, app_state: &AppState, area: Rect) {
    // Split the header area into sections
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30), // Title section
            Constraint::Min(1),     // Spacer
            Constraint::Length(25), // Network status
            Constraint::Length(45), // Wallet address
            Constraint::Length(15), // Block height
        ])
        .split(area);

    // Title section
    let title = Paragraph::new("🕉️  MANTRA DEX SDK")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(title, header_chunks[0]);

    // Network status section
    let network_status = get_network_status_text(app_state);
    let network_color = if app_state.network_info.is_syncing {
        Color::Yellow
    } else {
        Color::Green
    };

    let network = Paragraph::new(network_status)
        .style(Style::default().fg(network_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Network"),
        );
    f.render_widget(network, header_chunks[2]);

    // Wallet address section
    let wallet_text = app_state
        .wallet_address
        .as_ref()
        .map(|addr| {
            if addr.len() > 40 {
                format!("{}...{}", &addr[..8], &addr[addr.len() - 8..])
            } else {
                addr.clone()
            }
        })
        .unwrap_or_else(|| "Not Connected".to_string());

    let wallet_color = if app_state.wallet_address.is_some() {
        Color::Green
    } else {
        Color::Red
    };

    let wallet = Paragraph::new(wallet_text)
        .style(Style::default().fg(wallet_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Wallet"),
        );
    f.render_widget(wallet, header_chunks[3]);

    // Block height section
    let block_text = app_state
        .block_height
        .map(|height| height.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let block_height = Paragraph::new(block_text)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Block"),
        );
    f.render_widget(block_height, header_chunks[4]);
}

/// Get network status text with chain ID and sync status
fn get_network_status_text(app_state: &AppState) -> String {
    let default_chain_id = "Unknown".to_string();
    let chain_id = app_state
        .network_info
        .chain_id
        .as_ref()
        .unwrap_or(&default_chain_id);

    let sync_status = if app_state.network_info.is_syncing {
        "Syncing"
    } else {
        "Synced"
    };

    // Truncate chain ID if it's too long
    let display_chain_id = if chain_id.len() > 15 {
        format!("{}...", &chain_id[..12])
    } else {
        chain_id.clone()
    };

    format!("{}\n{}", display_chain_id, sync_status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_status_text() {
        let mut app_state = AppState::default();

        // Test with unknown chain ID and not syncing
        let status = get_network_status_text(&app_state);
        assert_eq!(status, "Unknown\nSynced");

        // Test with short chain ID (no truncation)
        app_state.network_info.chain_id = Some("mantra-1".to_string());
        app_state.network_info.is_syncing = false;
        let status = get_network_status_text(&app_state);
        assert_eq!(status, "mantra-1\nSynced");

        // Test with known chain ID and syncing (this will be truncated as it's 16 chars)
        app_state.network_info.chain_id = Some("mantra-hongbai-1".to_string());
        app_state.network_info.is_syncing = true;
        let status = get_network_status_text(&app_state);
        assert_eq!(status, "mantra-hongb...\nSyncing");

        // Test with long chain ID (longer than 15 characters)
        app_state.network_info.chain_id =
            Some("very-long-chain-id-name-here-with-more-text".to_string());
        let status = get_network_status_text(&app_state);
        assert!(status.starts_with("very-long-ch..."));
        assert!(status.contains("Syncing"));
    }
}
