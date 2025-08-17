use crossterm::event::{KeyCode, KeyEvent};
/// Simple list component that is always visible
/// Based on Ratatui's List widget for reliable functionality
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListEvent {
    Handled,
    Ignored,
    SelectionMade,
    SelectionCancelled,
}

#[derive(Debug, Clone)]
pub struct SimpleListOption {
    pub label: String,
    pub value: String,
}

impl SimpleListOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleList {
    /// Label for the list
    pub label: String,
    /// Available options
    pub options: Vec<SimpleListOption>,
    /// Currently selected option index
    pub selected_index: Option<usize>,
    /// Whether the list is the active component in the form
    pub is_active: bool,
    /// Whether the user is currently navigating the list options
    pub is_editing: bool,
    /// List state for rendering
    pub list_state: ListState,
}

impl SimpleList {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            options: Vec::new(),
            selected_index: None,
            is_active: false,
            is_editing: false,
            list_state: ListState::default(),
        }
    }

    pub fn with_options(mut self, options: Vec<SimpleListOption>) -> Self {
        self.options = options;
        if !self.options.is_empty() && self.selected_index.is_none() {
            // Initialize list state to highlight first item
            self.list_state.select(Some(0));
        }
        self
    }

    pub fn add_option(mut self, option: SimpleListOption) -> Self {
        self.options.push(option);
        if self.options.len() == 1 && self.selected_index.is_none() {
            self.list_state.select(Some(0));
        }
        self
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        if !active {
            self.is_editing = false; // Exit editing mode when not active
        }
    }

    pub fn get_selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|idx| self.options.get(idx))
            .map(|opt| opt.value.as_str())
    }

    pub fn get_selected_label(&self) -> Option<&str> {
        self.selected_index
            .and_then(|idx| self.options.get(idx))
            .map(|opt| opt.label.as_str())
    }

    pub fn has_options(&self) -> bool {
        !self.options.is_empty()
    }

    /// Handle key events directly - returns true if the event was handled
    pub fn handle_key_event(&mut self, key: KeyEvent) -> ListEvent {
        if !self.is_active {
            return ListEvent::Ignored;
        }

        if self.is_editing {
            match key.code {
                KeyCode::Enter => {
                    if !self.options.is_empty() {
                        if let Some(highlighted_idx) = self.list_state.selected() {
                            self.selected_index = Some(highlighted_idx);
                        }
                    }
                    self.is_editing = false;
                    ListEvent::SelectionMade
                }
                KeyCode::Esc => {
                    self.is_editing = false;
                    ListEvent::SelectionCancelled
                }
                KeyCode::Up => {
                    if !self.options.is_empty() {
                        let current = self.list_state.selected().unwrap_or(0);
                        if current > 0 {
                            self.list_state.select(Some(current - 1));
                        } else {
                            self.list_state.select(Some(self.options.len() - 1));
                        }
                    }
                    ListEvent::Handled
                }
                KeyCode::Down => {
                    if !self.options.is_empty() {
                        let current = self.list_state.selected().unwrap_or(0);
                        if current + 1 < self.options.len() {
                            self.list_state.select(Some(current + 1));
                        } else {
                            self.list_state.select(Some(0));
                        }
                    }
                    ListEvent::Handled
                }
                _ => ListEvent::Handled, // Absorb other keys to prevent them from propagating
            }
        } else {
            match key.code {
                KeyCode::Enter => {
                    if !self.options.is_empty() && self.is_active {
                        self.is_editing = true;
                        ListEvent::Handled
                    } else {
                        ListEvent::Ignored
                    }
                }
                // Allow up/down to work even when not in edit mode for quick navigation
                KeyCode::Up => {
                    if !self.options.is_empty() {
                        let current = self.list_state.selected().unwrap_or(0);
                        if current > 0 {
                            self.list_state.select(Some(current - 1));
                        } else {
                            self.list_state.select(Some(self.options.len() - 1));
                        }
                    }
                    ListEvent::Handled
                }
                KeyCode::Down => {
                    if !self.options.is_empty() {
                        let current = self.list_state.selected().unwrap_or(0);
                        if current + 1 < self.options.len() {
                            self.list_state.select(Some(current + 1));
                        } else {
                            self.list_state.select(Some(0));
                        }
                    }
                    ListEvent::Handled
                }
                _ => ListEvent::Ignored, // Let the parent handle other keys like Tab
            }
        }
    }

    /// Render the list (always visible)
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if self.options.is_empty() {
            // Render empty state
            let empty_list = List::new(vec![ListItem::new("No options available")]).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.label.as_str())
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
            f.render_widget(empty_list, area);
            return;
        }

        // Create list items
        let items: Vec<ListItem> = self
            .options
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                let text = if Some(idx) == self.selected_index {
                    format!("✓ {}", opt.label)
                } else {
                    format!("  {}", opt.label)
                };
                ListItem::new(text)
            })
            .collect();

        // Create and render the list
        let border_color = if self.is_editing {
            Color::Green // Green when in editing/selection mode
        } else if self.is_active {
            Color::Yellow // Yellow when focused but not editing
        } else {
            Color::Gray // Gray when not focused
        };

        let title = if self.is_editing {
            format!("{} [SELECTING]", self.label)
        } else if self.is_active {
            format!("{} [FOCUSED]", self.label)
        } else {
            self.label.clone()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.as_str())
                    .border_style(Style::default().fg(border_color)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}
