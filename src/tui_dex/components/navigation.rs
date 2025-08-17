//! Navigation Menu Component
//!
//! This component provides tab-based navigation between different screens
//! in the TUI application.

use crate::tui_dex::app::{AppState, Screen};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Tabs},
};

/// Render the navigation menu component
pub fn render_navigation(f: &mut Frame, app_state: &AppState, area: Rect) {
    let tabs = create_navigation_tabs(app_state);
    f.render_widget(tabs, area);
}

/// Create the navigation tabs widget
fn create_navigation_tabs(app_state: &AppState) -> Tabs {
    let screens = Screen::all();
    let titles: Vec<Line> = screens
        .iter()
        .map(|screen| {
            // Add keyboard shortcut indicators
            let shortcut_name = match screen {
                Screen::WalletSelection => "0:Wallet",
                Screen::Dashboard => "1:Dashboard",
                Screen::Pools => "2:Pools",
                Screen::Swap => "3:Swap",
                Screen::MultiHop => "4:Multi-hop",
                Screen::Liquidity => "5:Liquidity",
                Screen::Rewards => "6:Rewards",
                Screen::Admin => "7:Admin",
                Screen::Settings => "8:Settings",
                Screen::TransactionDetails => "9:Transaction",
            };
            Line::from(shortcut_name)
        })
        .collect();

    let selected_tab = screens
        .iter()
        .position(|&screen| screen == app_state.current_screen)
        .unwrap_or(0);

    let border_style = match app_state.navigation_mode {
        crate::tui_dex::app::NavigationMode::ScreenLevel => Style::default().fg(Color::Blue),
        crate::tui_dex::app::NavigationMode::WithinScreen => Style::default().fg(Color::Green),
    };

    // Create title based on navigation mode
    let title = match app_state.navigation_mode {
        crate::tui_dex::app::NavigationMode::ScreenLevel => "Navigation [TAB MODE]",
        crate::tui_dex::app::NavigationMode::WithinScreen => {
            if app_state.focus_manager.current_focus().is_some() {
                "Navigation [CONTENT MODE]"
            } else {
                "Navigation [CONTENT MODE]"
            }
        }
    };

    Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .select(selected_tab)
        .divider("|")
}

/// Get screen from navigation index
pub fn get_screen_from_index(index: usize) -> Option<Screen> {
    Screen::all().get(index).copied()
}

/// Get index from screen for navigation
pub fn get_index_from_screen(screen: Screen) -> usize {
    Screen::all().iter().position(|&s| s == screen).unwrap_or(0)
}

/// Get navigation help text
pub fn get_navigation_help() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Tab/→", "Next tab"),
        ("Shift+Tab/←", "Previous tab"),
        ("1-8", "Jump to screen"),
        ("q/Esc", "Exit app"),
        ("?", "Help"),
    ]
}

/// Check if a number key corresponds to a valid screen
pub fn number_key_to_screen(key: char) -> Option<Screen> {
    match key {
        '1' => Some(Screen::Dashboard),
        '2' => Some(Screen::Pools),
        '3' => Some(Screen::Swap),
        '4' => Some(Screen::MultiHop),
        '5' => Some(Screen::Liquidity),
        '6' => Some(Screen::Rewards),
        '7' => Some(Screen::Admin),
        '8' => Some(Screen::Settings),
        '9' => Some(Screen::TransactionDetails),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_screen_from_index() {
        assert_eq!(get_screen_from_index(0), Some(Screen::Dashboard));
        assert_eq!(get_screen_from_index(1), Some(Screen::Pools));
        assert_eq!(get_screen_from_index(2), Some(Screen::Swap));
        assert_eq!(get_screen_from_index(99), None);
    }

    #[test]
    fn test_get_index_from_screen() {
        assert_eq!(get_index_from_screen(Screen::Dashboard), 0);
        assert_eq!(get_index_from_screen(Screen::Pools), 1);
        assert_eq!(get_index_from_screen(Screen::Swap), 2);
    }

    #[test]
    fn test_number_key_to_screen() {
        assert_eq!(number_key_to_screen('1'), Some(Screen::Dashboard));
        assert_eq!(number_key_to_screen('2'), Some(Screen::Pools));
        assert_eq!(number_key_to_screen('8'), Some(Screen::Settings));
        assert_eq!(number_key_to_screen('9'), Some(Screen::TransactionDetails));
        assert_eq!(number_key_to_screen('a'), None);
    }
}
