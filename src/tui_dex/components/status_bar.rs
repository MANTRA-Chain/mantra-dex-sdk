//! Status Bar Component
//!
//! This component displays the bottom status bar with current action status,
//! error messages, loading indicators, and keyboard shortcuts.

use crate::tui_dex::app::{AppState, LoadingState};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

/// Render the status bar component
pub fn render_status_bar(f: &mut Frame, app_state: &AppState, area: Rect) {
    // Split status bar into sections
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Status/message area
            Constraint::Percentage(40), // Help/shortcuts area
        ])
        .split(area);

    // Render status/message section
    render_status_section(f, app_state, status_chunks[0]);

    // Render help/shortcuts section
    render_help_section(f, app_state, status_chunks[1]);
}

/// Render the status/message section
fn render_status_section(f: &mut Frame, app_state: &AppState, area: Rect) {
    match &app_state.loading_state {
        LoadingState::Loading {
            message, progress, ..
        } => {
            render_loading_status(f, message, area);
        }
        LoadingState::Success { message, .. } => {
            render_success_status(f, message, area);
        }
        LoadingState::Error { message, .. } => {
            render_error_status(f, message, area);
        }
        LoadingState::Idle => {
            if let Some(error) = &app_state.error_message {
                render_error_status(f, error, area);
            } else if let Some(status) = &app_state.status_message {
                render_normal_status(f, status, area);
            } else {
                render_default_status(f, app_state, area);
            }
        }
    }
}

/// Render loading status with progress indicator
fn render_loading_status(f: &mut Frame, message: &str, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Loading message
    let loading_text = Paragraph::new(format!("⏳ {}", message))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Status"),
        );
    f.render_widget(loading_text, chunks[0]);

    // Animated progress bar (simplified for this implementation)
    let progress = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Yellow))
        .ratio(0.5); // Could be animated based on time
    f.render_widget(progress, chunks[1]);
}

/// Render success status
fn render_success_status(f: &mut Frame, message: &str, area: Rect) {
    let success_text = Paragraph::new(format!("✅ {}", message))
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title("Success"),
        );
    f.render_widget(success_text, area);
}

/// Render error status
fn render_error_status(f: &mut Frame, message: &str, area: Rect) {
    let error_text = Paragraph::new(format!("❌ {}", message))
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title("Error"),
        );
    f.render_widget(error_text, area);
}

/// Render normal status message
fn render_normal_status(f: &mut Frame, message: &str, area: Rect) {
    let status_text = Paragraph::new(format!("ℹ️ {}", message))
        .style(Style::default().fg(Color::Blue))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Status"),
        );
    f.render_widget(status_text, area);
}

/// Render default status when no specific message
fn render_default_status(f: &mut Frame, app_state: &AppState, area: Rect) {
    let default_message = format!(
        "Ready | Screen: {} | Connected: {}",
        app_state.current_screen.display_name(),
        if app_state.wallet_address.is_some() {
            "Yes"
        } else {
            "No"
        }
    );

    let status_text = Paragraph::new(default_message)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Status"),
        );
    f.render_widget(status_text, area);
}

/// Render help/shortcuts section
fn render_help_section(f: &mut Frame, app_state: &AppState, area: Rect) {
    let help_text = get_context_help(app_state);

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Help"),
        );
    f.render_widget(help, area);
}

/// Get context-sensitive help text based on current screen
fn get_context_help(app_state: &AppState) -> String {
    let base_help = "Tab:Next | Shift+Tab:Prev | q:Quit | ?:Help";

    let screen_help = match app_state.current_screen {
        crate::tui_dex::app::Screen::WalletSelection => {
            "↑↓:Select | Enter:Load | n:New | r:Recover"
        }
        crate::tui_dex::app::Screen::Dashboard => "Enter:Refresh",
        crate::tui_dex::app::Screen::Pools => "↑↓:Select | Enter:Details | r:Refresh",
        crate::tui_dex::app::Screen::Swap => "Enter:Execute | s:Simulate | r:Reset",
        crate::tui_dex::app::Screen::MultiHop => "a:Add hop | d:Delete | Enter:Execute",
        crate::tui_dex::app::Screen::Liquidity => "p:Provide | w:Withdraw | Enter:Execute",
        crate::tui_dex::app::Screen::Rewards => "c:Claim | a:Claim all | Enter:Details",
        crate::tui_dex::app::Screen::Admin => "n:New pool | e:Edit | t:Toggle",
        crate::tui_dex::app::Screen::Settings => "s:Save | r:Reset | Enter:Edit",
        crate::tui_dex::app::Screen::TransactionDetails => "Esc:Back | r:Refresh",
    };

    format!("{} | {}", base_help, screen_help)
}

/// Get loading indicator character based on time (for animation)
pub fn get_loading_indicator(time_ms: u64) -> char {
    let indicators = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let index = (time_ms / 100) % indicators.len() as u64;
    indicators[index as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_loading_indicator() {
        assert_eq!(get_loading_indicator(0), '⠋');
        assert_eq!(get_loading_indicator(100), '⠙');
        assert_eq!(get_loading_indicator(1000), '⠋'); // Should cycle back
    }

    #[test]
    fn test_get_context_help() {
        let mut app_state = AppState::default();

        // Test dashboard help
        app_state.current_screen = crate::tui_dex::app::Screen::Dashboard;
        let help = get_context_help(&app_state);
        assert!(help.contains("Enter:Refresh"));

        // Test pools help
        app_state.current_screen = crate::tui_dex::app::Screen::Pools;
        let help = get_context_help(&app_state);
        assert!(help.contains("↑↓:Select"));
    }
}
