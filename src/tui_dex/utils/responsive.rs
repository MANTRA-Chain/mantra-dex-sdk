//! Responsive Layout System for TUI
//!
//! This module provides utilities for creating responsive layouts that adapt
//! to different terminal sizes while maintaining usability and minimum size requirements.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// Minimum terminal dimensions for the application
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

/// Recommended terminal dimensions for optimal experience
pub const RECOMMENDED_WIDTH: u16 = 120;
pub const RECOMMENDED_HEIGHT: u16 = 30;

/// Layout modes based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Compact layout for minimum supported size (80x24)
    Compact,
    /// Normal layout for medium terminals (120x30)
    Normal,
    /// Expanded layout for large terminals (140x35+)
    Expanded,
}

/// Screen size category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenSize {
    /// Terminal too small to use effectively
    TooSmall,
    /// Minimum usable size
    Small,
    /// Standard comfortable size
    Medium,
    /// Large terminal with extra space
    Large,
    /// Extra large terminal
    ExtraLarge,
}

/// Layout configuration for different screen components
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub mode: LayoutMode,
    pub size: ScreenSize,
    pub area: Rect,
    pub show_details: bool,
    pub show_help: bool,
    pub compact_tables: bool,
    pub max_table_rows: usize,
    pub show_sidebar: bool,
}

impl LayoutConfig {
    /// Create layout configuration based on terminal size
    pub fn new(area: Rect) -> Self {
        let size = Self::categorize_size(area);
        let mode = Self::determine_mode(area);

        Self {
            mode,
            size,
            area,
            show_details: matches!(
                size,
                ScreenSize::Medium | ScreenSize::Large | ScreenSize::ExtraLarge
            ),
            show_help: matches!(size, ScreenSize::Large | ScreenSize::ExtraLarge),
            compact_tables: matches!(size, ScreenSize::Small | ScreenSize::TooSmall),
            max_table_rows: Self::calculate_max_table_rows(area),
            show_sidebar: matches!(size, ScreenSize::Large | ScreenSize::ExtraLarge)
                && area.width >= 140,
        }
    }

    /// Categorize terminal size
    fn categorize_size(area: Rect) -> ScreenSize {
        match (area.width, area.height) {
            (w, h) if w < MIN_WIDTH || h < MIN_HEIGHT => ScreenSize::TooSmall,
            (w, h) if w < 100 || h < 28 => ScreenSize::Small,
            (w, h) if w < 140 || h < 35 => ScreenSize::Medium,
            (w, h) if w < 180 || h < 45 => ScreenSize::Large,
            _ => ScreenSize::ExtraLarge,
        }
    }

    /// Determine layout mode based on size
    fn determine_mode(area: Rect) -> LayoutMode {
        match (area.width, area.height) {
            (w, h) if w >= 140 && h >= 35 => LayoutMode::Expanded,
            (w, h) if w >= RECOMMENDED_WIDTH && h >= RECOMMENDED_HEIGHT => LayoutMode::Normal,
            _ => LayoutMode::Compact,
        }
    }

    /// Calculate maximum table rows based on available space
    fn calculate_max_table_rows(area: Rect) -> usize {
        // Reserve space for header (3), navigation (3), status (3), borders (2)
        let available_height = area.height.saturating_sub(11) as usize;
        std::cmp::max(5, available_height.saturating_sub(5)) // At least 5 rows, leave 5 for other content
    }

    /// Check if terminal is too small to use effectively
    pub fn is_too_small(&self) -> bool {
        matches!(self.size, ScreenSize::TooSmall)
    }

    /// Get constraints for main layout based on layout mode
    pub fn main_layout_constraints(&self) -> Vec<Constraint> {
        match self.mode {
            LayoutMode::Compact => vec![
                Constraint::Length(3), // Header (compact)
                Constraint::Min(0),    // Body
                Constraint::Length(2), // Status (compact)
            ],
            LayoutMode::Normal => vec![
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Body
                Constraint::Length(3), // Status
            ],
            LayoutMode::Expanded => vec![
                Constraint::Length(4), // Header (expanded)
                Constraint::Min(0),    // Body
                Constraint::Length(4), // Status (expanded)
            ],
        }
    }

    /// Get constraints for content layout (body with optional sidebar)
    pub fn content_layout_constraints(&self) -> Vec<Constraint> {
        if self.show_sidebar {
            vec![
                Constraint::Min(0),     // Main content
                Constraint::Length(30), // Sidebar
            ]
        } else {
            vec![Constraint::Min(0)] // Full width content
        }
    }

    /// Get constraints for form layouts based on available space
    pub fn form_layout_constraints(&self) -> Vec<Constraint> {
        match self.mode {
            LayoutMode::Compact => vec![
                Constraint::Length(3), // Single input row
                Constraint::Length(3), // Single button row
            ],
            LayoutMode::Normal => vec![
                Constraint::Length(4), // Input section
                Constraint::Length(6), // Details section
                Constraint::Length(3), // Button section
            ],
            LayoutMode::Expanded => vec![
                Constraint::Length(6), // Input section
                Constraint::Length(8), // Details section
                Constraint::Length(4), // Button section
                Constraint::Length(4), // Help section
            ],
        }
    }

    /// Get table constraints based on layout mode
    pub fn table_constraints(&self) -> (Vec<Constraint>, bool) {
        let show_all_columns = !self.compact_tables;

        match (self.mode, self.compact_tables) {
            (LayoutMode::Compact, true) => (
                vec![
                    Constraint::Length(8),  // ID
                    Constraint::Min(20),    // Primary info
                    Constraint::Length(12), // Status
                ],
                show_all_columns,
            ),
            (LayoutMode::Normal, _) => (
                vec![
                    Constraint::Length(8),  // ID
                    Constraint::Min(15),    // Asset 1
                    Constraint::Min(15),    // Asset 2
                    Constraint::Length(12), // TVL
                    Constraint::Length(8),  // APY
                    Constraint::Length(12), // Status
                ],
                show_all_columns,
            ),
            (LayoutMode::Expanded, _) => (
                vec![
                    Constraint::Length(8),  // ID
                    Constraint::Min(15),    // Asset 1
                    Constraint::Min(15),    // Asset 2
                    Constraint::Length(15), // TVL
                    Constraint::Length(10), // APY
                    Constraint::Length(12), // Volume
                    Constraint::Length(10), // Fees
                    Constraint::Length(12), // Status
                ],
                show_all_columns,
            ),
            _ => (
                vec![
                    Constraint::Length(8),  // ID
                    Constraint::Min(25),    // Combined info
                    Constraint::Length(12), // Status
                ],
                show_all_columns,
            ),
        }
    }

    /// Get appropriate text wrap width for content
    pub fn text_wrap_width(&self) -> usize {
        match self.mode {
            LayoutMode::Compact => 60,
            LayoutMode::Normal => 80,
            LayoutMode::Expanded => 100,
        }
    }

    /// Check if detailed information should be shown
    pub fn show_detailed_info(&self) -> bool {
        self.show_details && matches!(self.mode, LayoutMode::Normal | LayoutMode::Expanded)
    }

    /// Get number of columns for grid layouts
    pub fn grid_columns(&self) -> usize {
        match self.mode {
            LayoutMode::Compact => 1,
            LayoutMode::Normal => 2,
            LayoutMode::Expanded => 3,
        }
    }
}

/// Create a warning popup for terminals that are too small
pub fn create_size_warning_popup(
    area: Rect,
) -> (
    Rect,
    ratatui::widgets::Clear,
    ratatui::widgets::Paragraph<'static>,
) {
    let popup_area = popup_area(area, 60, 12);

    let warning_text = vec![
        Line::from("⚠️  Terminal Too Small ⚠️"),
        Line::from(""),
        Line::from(format!("Current size: {}x{}", area.width, area.height)),
        Line::from(format!("Minimum required: {}x{}", MIN_WIDTH, MIN_HEIGHT)),
        Line::from(format!(
            "Recommended: {}x{}",
            RECOMMENDED_WIDTH, RECOMMENDED_HEIGHT
        )),
        Line::from(""),
        Line::from("Please resize your terminal for the best experience."),
        Line::from("Press 'q' to quit or any other key to continue."),
    ];

    let warning = Paragraph::new(warning_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title("Size Warning")
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        );

    (popup_area, ratatui::widgets::Clear, warning)
}

/// Helper function to create a centered popup area
pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Adaptive spacing based on layout mode
pub fn adaptive_spacing(config: &LayoutConfig) -> u16 {
    match config.mode {
        LayoutMode::Compact => 0,
        LayoutMode::Normal => 1,
        LayoutMode::Expanded => 2,
    }
}

/// Calculate adaptive margins
pub fn adaptive_margins(config: &LayoutConfig) -> Margin {
    match config.mode {
        LayoutMode::Compact => Margin {
            horizontal: 0,
            vertical: 0,
        },
        LayoutMode::Normal => Margin {
            horizontal: 1,
            vertical: 0,
        },
        LayoutMode::Expanded => Margin {
            horizontal: 2,
            vertical: 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_config_creation() {
        // Test compact layout
        let small_area = Rect::new(0, 0, 80, 24);
        let config = LayoutConfig::new(small_area);
        assert_eq!(config.mode, LayoutMode::Compact);
        assert_eq!(config.size, ScreenSize::Small);

        // Test normal layout
        let medium_area = Rect::new(0, 0, 120, 30);
        let config = LayoutConfig::new(medium_area);
        assert_eq!(config.mode, LayoutMode::Normal);
        assert_eq!(config.size, ScreenSize::Medium);

        // Test expanded layout
        let large_area = Rect::new(0, 0, 150, 40);
        let config = LayoutConfig::new(large_area);
        assert_eq!(config.mode, LayoutMode::Expanded);
        assert_eq!(config.size, ScreenSize::Large);
    }

    #[test]
    fn test_size_categorization() {
        // Too small
        let tiny_area = Rect::new(0, 0, 70, 20);
        let config = LayoutConfig::new(tiny_area);
        assert!(config.is_too_small());

        // Minimum size
        let min_area = Rect::new(0, 0, MIN_WIDTH, MIN_HEIGHT);
        let config = LayoutConfig::new(min_area);
        assert!(!config.is_too_small());
        assert_eq!(config.size, ScreenSize::Small);
    }

    #[test]
    fn test_table_constraints() {
        let compact_area = Rect::new(0, 0, 80, 24);
        let config = LayoutConfig::new(compact_area);
        let (constraints, show_all) = config.table_constraints();
        assert_eq!(constraints.len(), 3); // Compact table
        assert!(!show_all); // Should not show all columns

        let large_area = Rect::new(0, 0, 150, 40);
        let config = LayoutConfig::new(large_area);
        let (constraints, show_all) = config.table_constraints();
        assert!(constraints.len() > 3); // Expanded table
        assert!(show_all); // Should show all columns
    }

    #[test]
    fn test_adaptive_features() {
        let small_config = LayoutConfig::new(Rect::new(0, 0, 80, 24));
        assert!(!small_config.show_detailed_info());
        assert!(!small_config.show_sidebar);
        assert_eq!(small_config.grid_columns(), 1);

        let large_config = LayoutConfig::new(Rect::new(0, 0, 150, 40));
        assert!(large_config.show_detailed_info());
        assert_eq!(large_config.grid_columns(), 3);
    }
}
