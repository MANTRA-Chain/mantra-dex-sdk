//! Data Visualization Components
//!
//! This module contains advanced visualization components like progress bars,
//! charts, and other data display widgets for the TUI application.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use std::time::SystemTime;

use crate::tui_dex::app::{AppState, LoadingState, TransactionInfo, TransactionStatus};

/// Progress bar styles for different types of operations
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressBarStyle {
    /// Transaction confirmation progress
    TransactionConfirmation,
    /// Network synchronization progress  
    NetworkSync,
    /// Generic loading progress
    Loading,
    /// Success state
    Success,
    /// Error state
    Error,
}

/// Enhanced progress bar configuration
#[derive(Debug, Clone)]
pub struct ProgressBarConfig {
    pub title: String,
    pub message: String,
    pub progress: f64, // 0.0 to 100.0
    pub style: ProgressBarStyle,
    pub show_percentage: bool,
    pub show_eta: bool,
    pub started_at: Option<SystemTime>,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            title: "Progress".to_string(),
            message: "Processing...".to_string(),
            progress: 0.0,
            style: ProgressBarStyle::Loading,
            show_percentage: true,
            show_eta: false,
            started_at: None,
        }
    }
}

impl ProgressBarConfig {
    /// Create a transaction confirmation progress bar
    pub fn transaction_confirmation(
        tx_hash: &str,
        confirmations: u32,
        required_confirmations: u32,
    ) -> Self {
        let progress = (confirmations as f64 / required_confirmations as f64 * 100.0).min(100.0);
        let short_hash = if tx_hash.len() > 12 {
            format!("{}...{}", &tx_hash[..6], &tx_hash[tx_hash.len() - 6..])
        } else {
            tx_hash.to_string()
        };

        Self {
            title: "Transaction Confirmation".to_string(),
            message: format!(
                "Confirming {} ({}/{})",
                short_hash, confirmations, required_confirmations
            ),
            progress,
            style: ProgressBarStyle::TransactionConfirmation,
            show_percentage: true,
            show_eta: true,
            started_at: Some(SystemTime::now()),
        }
    }

    /// Create a network sync progress bar
    pub fn network_sync(current_block: u64, target_block: u64, is_synced: bool) -> Self {
        let progress = if is_synced {
            100.0
        } else if target_block > 0 {
            (current_block as f64 / target_block as f64 * 100.0).min(100.0)
        } else {
            0.0
        };

        Self {
            title: "Network Synchronization".to_string(),
            message: if is_synced {
                "Network synchronized".to_string()
            } else {
                format!("Syncing blocks ({}/{})", current_block, target_block)
            },
            progress,
            style: ProgressBarStyle::NetworkSync,
            show_percentage: true,
            show_eta: !is_synced,
            started_at: Some(SystemTime::now()),
        }
    }

    /// Create a generic loading progress bar
    pub fn loading(title: &str, message: &str, progress: f64) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            progress: progress.clamp(0.0, 100.0),
            style: ProgressBarStyle::Loading,
            show_percentage: true,
            show_eta: false,
            started_at: Some(SystemTime::now()),
        }
    }

    /// Update progress and message
    pub fn update(&mut self, progress: f64, message: Option<String>) {
        self.progress = progress.clamp(0.0, 100.0);
        if let Some(msg) = message {
            self.message = msg;
        }
    }

    /// Get estimated time remaining (if available)
    pub fn estimated_time_remaining(&self) -> Option<u64> {
        if !self.show_eta || self.progress <= 0.0 || self.progress >= 100.0 {
            return None;
        }

        if let Some(started_at) = self.started_at {
            if let Ok(elapsed) = started_at.elapsed() {
                let elapsed_secs = elapsed.as_secs();
                if elapsed_secs > 0 {
                    let rate = self.progress / elapsed_secs as f64;
                    let remaining_progress = 100.0 - self.progress;
                    let eta_secs = (remaining_progress / rate) as u64;
                    return Some(eta_secs);
                }
            }
        }
        None
    }

    /// Format ETA as human-readable string
    pub fn format_eta(&self) -> Option<String> {
        if let Some(eta_secs) = self.estimated_time_remaining() {
            if eta_secs < 60 {
                Some(format!("{}s", eta_secs))
            } else if eta_secs < 3600 {
                Some(format!("{}m {}s", eta_secs / 60, eta_secs % 60))
            } else {
                Some(format!("{}h {}m", eta_secs / 3600, (eta_secs % 3600) / 60))
            }
        } else {
            None
        }
    }
}

/// Render an enhanced progress bar with advanced features
pub fn render_enhanced_progress_bar(f: &mut Frame, config: &ProgressBarConfig, area: Rect) {
    let (gauge_color, border_color) = match config.style {
        ProgressBarStyle::TransactionConfirmation => (Color::Cyan, Color::Cyan),
        ProgressBarStyle::NetworkSync => (Color::Blue, Color::Blue),
        ProgressBarStyle::Loading => (Color::Yellow, Color::Yellow),
        ProgressBarStyle::Success => (Color::Green, Color::Green),
        ProgressBarStyle::Error => (Color::Red, Color::Red),
    };

    // Create layout for progress bar and optional ETA
    let chunks = if config.show_eta && config.estimated_time_remaining().is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    // Create the main progress bar
    let progress_percent = config.progress.clamp(0.0, 100.0) as u16;

    let label = if config.show_percentage {
        format!("{} ({}%)", config.message, progress_percent)
    } else {
        config.message.clone()
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(config.title.clone()),
        )
        .gauge_style(Style::default().fg(gauge_color))
        .percent(progress_percent)
        .label(label);

    f.render_widget(gauge, chunks[0]);

    // Render ETA if available
    if chunks.len() > 1 {
        if let Some(eta) = config.format_eta() {
            let eta_text = Paragraph::new(format!("ETA: {}", eta))
                .style(Style::default().fg(Color::Gray))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(eta_text, chunks[1]);
        }
    }
}

/// Render multiple progress bars in a stacked layout
pub fn render_progress_stack(f: &mut Frame, configs: &[ProgressBarConfig], area: Rect) {
    if configs.is_empty() {
        return;
    }

    let constraints: Vec<Constraint> = configs
        .iter()
        .map(|config| {
            if config.show_eta && config.estimated_time_remaining().is_some() {
                Constraint::Length(4) // 3 for gauge + 1 for ETA
            } else {
                Constraint::Length(3) // Just the gauge
            }
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, config) in configs.iter().enumerate() {
        if i < chunks.len() {
            render_enhanced_progress_bar(f, config, chunks[i]);
        }
    }
}

/// Render transaction confirmation progress for multiple transactions
pub fn render_transaction_confirmation_progress(
    f: &mut Frame,
    transactions: &[TransactionInfo],
    area: Rect,
) {
    let pending_transactions: Vec<_> = transactions
        .iter()
        .filter(|tx| tx.status == TransactionStatus::Pending)
        .collect();

    if pending_transactions.is_empty() {
        // Show a message when no pending transactions
        let message = Paragraph::new("No pending transactions")
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Transaction Progress")
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(message, area);
        return;
    }

    // Create progress bars for pending transactions
    let configs: Vec<ProgressBarConfig> = pending_transactions
        .into_iter()
        .take(5) // Limit to 5 most recent pending transactions
        .map(|tx| {
            // Simulate confirmation progress based on time elapsed
            let elapsed = chrono::Utc::now().signed_duration_since(tx.timestamp);
            let elapsed_secs = elapsed.num_seconds().max(0) as u64;
            let simulated_confirmations = (elapsed_secs / 15).min(6) as u32; // ~15 seconds per confirmation
            let required_confirmations = 6u32;

            ProgressBarConfig::transaction_confirmation(
                &tx.hash,
                simulated_confirmations,
                required_confirmations,
            )
        })
        .collect();

    render_progress_stack(f, &configs, area);
}

/// Render network synchronization progress
pub fn render_network_sync_progress(f: &mut Frame, app_state: &AppState, area: Rect) {
    let network_info = &app_state.network_info;

    // Use block height from app state, simulate target block height
    let current_block = app_state.block_height.unwrap_or(0);
    let target_block = current_block + if network_info.is_syncing { 100 } else { 0 }; // Simulate target

    let config =
        ProgressBarConfig::network_sync(current_block, target_block, !network_info.is_syncing);

    render_enhanced_progress_bar(f, &config, area);
}

/// Render combined progress dashboard
pub fn render_progress_dashboard(f: &mut Frame, app_state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Network sync
            Constraint::Percentage(50), // Transaction confirmations
        ])
        .split(area);

    // Render network sync progress
    render_network_sync_progress(f, app_state, chunks[0]);

    // Render transaction confirmation progress
    render_transaction_confirmation_progress(f, &app_state.recent_transactions, chunks[1]);
}

/// Render loading state with enhanced progress bar
pub fn render_loading_progress(f: &mut Frame, loading_state: &LoadingState, area: Rect) {
    let config = match loading_state {
        LoadingState::Idle => return, // Don't render anything when idle
        LoadingState::Loading {
            message, progress, ..
        } => {
            let prog = progress.unwrap_or(50.0); // Default to 50% for indeterminate
            ProgressBarConfig::loading("Loading", message, prog)
        }
        LoadingState::Success { message, .. } => {
            let mut config = ProgressBarConfig::loading("Success", message, 100.0);
            config.style = ProgressBarStyle::Success;
            config
        }
        LoadingState::Error { message, .. } => {
            let mut config = ProgressBarConfig::loading("Error", message, 0.0);
            config.style = ProgressBarStyle::Error;
            config
        }
    };

    render_enhanced_progress_bar(f, &config, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_progress_bar_config_creation() {
        let config = ProgressBarConfig::transaction_confirmation("0x1234567890abcdef", 3, 6);
        assert_eq!(config.progress, 50.0);
        assert_eq!(config.style, ProgressBarStyle::TransactionConfirmation);
        assert!(config.show_percentage);
        assert!(config.show_eta);
    }

    #[test]
    fn test_network_sync_progress() {
        let config = ProgressBarConfig::network_sync(500, 1000, false);
        assert_eq!(config.progress, 50.0);
        assert_eq!(config.style, ProgressBarStyle::NetworkSync);

        let synced_config = ProgressBarConfig::network_sync(1000, 1000, true);
        assert_eq!(synced_config.progress, 100.0);
    }

    #[test]
    fn test_progress_update() {
        let mut config = ProgressBarConfig::default();
        config.update(75.0, Some("Updated message".to_string()));
        assert_eq!(config.progress, 75.0);
        assert_eq!(config.message, "Updated message");
    }

    #[test]
    fn test_eta_calculation() {
        let mut config = ProgressBarConfig::default();
        config.started_at = Some(SystemTime::now() - Duration::from_secs(10));
        config.progress = 25.0;
        config.show_eta = true;

        let eta = config.estimated_time_remaining();
        assert!(eta.is_some());
        assert!(eta.unwrap() > 0);
    }

    #[test]
    fn test_eta_formatting() {
        let mut config = ProgressBarConfig::default();
        config.started_at = Some(SystemTime::now() - Duration::from_secs(10));
        config.progress = 50.0;
        config.show_eta = true;

        let formatted_eta = config.format_eta();
        assert!(formatted_eta.is_some());
        assert!(formatted_eta.unwrap().contains("s"));
    }

    #[test]
    fn test_progress_clamping() {
        let mut config = ProgressBarConfig::default();

        // Test upper bound
        config.update(150.0, None);
        assert_eq!(config.progress, 100.0);

        // Test lower bound
        config.update(-50.0, None);
        assert_eq!(config.progress, 0.0);
    }
}
