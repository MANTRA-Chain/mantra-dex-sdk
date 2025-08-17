//! Focus Management System for TUI Components
//!
//! This module provides a comprehensive focus management system that handles
//! keyboard navigation, tab order, and focus state across all TUI components.
//! It ensures consistent navigation behavior and accessibility compliance.

use crate::tui_dex::events::{Event, FocusDirection, FocusableComponent};
use std::collections::HashMap;

/// Focus manager handles focus state and navigation across components
#[derive(Debug, Clone)]
pub struct FocusManager {
    /// Current focused component
    current_focus: Option<FocusableComponent>,
    /// Ordered list of focusable components for tab navigation
    tab_order: Vec<FocusableComponent>,
    /// Component visibility state (used to skip hidden components)
    component_visibility: HashMap<FocusableComponent, bool>,
    /// Component enabled state (used to skip disabled components)
    component_enabled: HashMap<FocusableComponent, bool>,
    /// Wrap around behavior (focus cycles from last to first)
    wrap_around: bool,
    /// Focus history for returning to previous focus
    focus_history: Vec<FocusableComponent>,
    /// Maximum history size
    max_history: usize,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    /// Create a new focus manager
    pub fn new() -> Self {
        Self {
            current_focus: None,
            tab_order: Vec::new(),
            component_visibility: HashMap::new(),
            component_enabled: HashMap::new(),
            wrap_around: true,
            focus_history: Vec::new(),
            max_history: 10,
        }
    }

    /// Set the tab order for components
    pub fn set_tab_order(&mut self, components: Vec<FocusableComponent>) {
        self.tab_order = components;
        // Initialize all components as visible and enabled by default
        for component in &self.tab_order {
            self.component_visibility.insert(component.clone(), true);
            self.component_enabled.insert(component.clone(), true);
        }
    }

    /// Add a component to the tab order
    pub fn add_component(&mut self, component: FocusableComponent) {
        if !self.tab_order.contains(&component) {
            self.tab_order.push(component.clone());
            self.component_visibility.insert(component.clone(), true);
            self.component_enabled.insert(component, true);
        }
    }

    /// Remove a component from the tab order
    pub fn remove_component(&mut self, component: &FocusableComponent) {
        self.tab_order.retain(|c| c != component);
        self.component_visibility.remove(component);
        self.component_enabled.remove(component);

        // Clear focus if the removed component was focused
        if self.current_focus.as_ref() == Some(component) {
            self.current_focus = None;
        }
    }

    /// Set component visibility (hidden components are skipped in navigation)
    pub fn set_component_visibility(&mut self, component: FocusableComponent, visible: bool) {
        self.component_visibility.insert(component, visible);
    }

    /// Set component enabled state (disabled components are skipped in navigation)
    pub fn set_component_enabled(&mut self, component: FocusableComponent, enabled: bool) {
        self.component_enabled.insert(component, enabled);
    }

    /// Get the currently focused component
    pub fn current_focus(&self) -> Option<&FocusableComponent> {
        self.current_focus.as_ref()
    }

    /// Check if a component is currently focused
    pub fn is_focused(&self, component: &FocusableComponent) -> bool {
        self.current_focus.as_ref() == Some(component)
    }

    /// Set focus to a specific component
    pub fn set_focus(&mut self, component: FocusableComponent) -> bool {
        if self.is_component_focusable(&component) {
            self.add_to_history();
            self.current_focus = Some(component);
            true
        } else {
            false
        }
    }

    /// Clear all focus
    pub fn clear_focus(&mut self) {
        self.add_to_history();
        self.current_focus = None;
    }

    /// Move focus in the specified direction
    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<FocusableComponent> {
        match direction {
            FocusDirection::Next => self.focus_next(),
            FocusDirection::Previous => self.focus_previous(),
            FocusDirection::First => self.focus_first(),
            FocusDirection::Last => self.focus_last(),
            FocusDirection::Up
            | FocusDirection::Down
            | FocusDirection::Left
            | FocusDirection::Right => {
                // For directional navigation, we can implement custom logic
                // For now, treat as next/previous
                match direction {
                    FocusDirection::Up | FocusDirection::Left => self.focus_previous(),
                    FocusDirection::Down | FocusDirection::Right => self.focus_next(),
                    _ => None,
                }
            }
        }
    }

    /// Focus the next component in tab order
    pub fn focus_next(&mut self) -> Option<FocusableComponent> {
        if self.tab_order.is_empty() {
            return None;
        }

        let current_index = self.current_focus_index();
        let start_index = current_index.map(|i| i + 1).unwrap_or(0);

        // Find next focusable component
        for i in 0..self.tab_order.len() {
            let index = (start_index + i) % self.tab_order.len();
            let component = self.tab_order[index].clone();

            if self.is_component_focusable(&component) {
                self.add_to_history();
                self.current_focus = Some(component.clone());
                return Some(component);
            }
        }

        None
    }

    /// Focus the previous component in tab order
    pub fn focus_previous(&mut self) -> Option<FocusableComponent> {
        if self.tab_order.is_empty() {
            return None;
        }

        let start_index = self.current_focus_index().unwrap_or(0);

        // Find previous focusable component by iterating backwards
        for i in 1..=self.tab_order.len() {
            let index = (start_index + self.tab_order.len() - i) % self.tab_order.len();
            let component = self.tab_order[index].clone();

            if self.is_component_focusable(&component) {
                self.add_to_history();
                self.current_focus = Some(component.clone());
                return Some(component.clone());
            }
        }

        None
    }

    /// Focus the first focusable component
    pub fn focus_first(&mut self) -> Option<FocusableComponent> {
        for component in self.tab_order.clone() {
            if self.is_component_focusable(&component) {
                self.add_to_history();
                self.current_focus = Some(component.clone());
                return Some(component);
            }
        }
        None
    }

    /// Focus the last focusable component
    pub fn focus_last(&mut self) -> Option<FocusableComponent> {
        let tab_order_clone = self.tab_order.clone();
        for component in tab_order_clone.iter().rev() {
            if self.is_component_focusable(component) {
                self.add_to_history();
                self.current_focus = Some(component.clone());
                return Some(component.clone());
            }
        }
        None
    }

    /// Return to the previous focus (undo last focus change)
    pub fn return_to_previous(&mut self) -> Option<FocusableComponent> {
        if let Some(previous) = self.focus_history.pop() {
            if self.is_component_focusable(&previous) {
                self.current_focus = Some(previous.clone());
                return Some(previous);
            }
        }
        None
    }

    /// Handle focus-related events
    pub fn handle_event(&mut self, event: &Event) -> Option<FocusableComponent> {
        match event {
            Event::FocusNext => self.focus_next(),
            Event::FocusPrevious => self.focus_previous(),
            Event::FocusFirst => self.focus_first(),
            Event::FocusLast => self.focus_last(),
            Event::MoveFocus(direction) => self.move_focus(direction.clone()),
            Event::SetFocus(component) => {
                if self.set_focus(component.clone()) {
                    Some(component.clone())
                } else {
                    None
                }
            }
            Event::ClearFocus => {
                self.clear_focus();
                None
            }
            Event::Escape => {
                // Escape key can be used to return to previous focus or clear focus
                self.return_to_previous().or_else(|| {
                    self.clear_focus();
                    None
                })
            }
            _ => None,
        }
    }

    /// Set wrap around behavior
    pub fn set_wrap_around(&mut self, wrap: bool) {
        self.wrap_around = wrap;
    }

    /// Get all focusable components in order
    pub fn get_focusable_components(&self) -> Vec<FocusableComponent> {
        self.tab_order
            .iter()
            .filter(|c| self.is_component_focusable(c))
            .cloned()
            .collect()
    }

    /// Get focus statistics for debugging
    pub fn get_focus_stats(&self) -> FocusStats {
        let total_components = self.tab_order.len();
        let visible_components = self
            .tab_order
            .iter()
            .filter(|c| *self.component_visibility.get(c).unwrap_or(&true))
            .count();
        let enabled_components = self
            .tab_order
            .iter()
            .filter(|c| *self.component_enabled.get(c).unwrap_or(&true))
            .count();
        let focusable_components = self.get_focusable_components().len();

        FocusStats {
            total_components,
            visible_components,
            enabled_components,
            focusable_components,
            current_focus: self.current_focus.clone(),
            history_size: self.focus_history.len(),
        }
    }

    /// Check if a component can receive focus
    fn is_component_focusable(&self, component: &FocusableComponent) -> bool {
        let visible = self.component_visibility.get(component).unwrap_or(&true);
        let enabled = self.component_enabled.get(component).unwrap_or(&true);
        *visible && *enabled
    }

    /// Get the index of the currently focused component
    fn current_focus_index(&self) -> Option<usize> {
        if let Some(focus) = &self.current_focus {
            self.tab_order.iter().position(|c| c == focus)
        } else {
            None
        }
    }

    /// Add current focus to history before changing focus
    fn add_to_history(&mut self) {
        if let Some(current) = &self.current_focus {
            self.focus_history.push(current.clone());

            // Limit history size
            if self.focus_history.len() > self.max_history {
                self.focus_history.remove(0);
            }
        }
    }
}

/// Focus statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct FocusStats {
    pub total_components: usize,
    pub visible_components: usize,
    pub enabled_components: usize,
    pub focusable_components: usize,
    pub current_focus: Option<FocusableComponent>,
    pub history_size: usize,
}

/// Helper function to create common component IDs
pub mod component_ids {
    use crate::tui_dex::events::FocusableComponent;

    // Dashboard components
    pub fn dashboard_refresh_button() -> FocusableComponent {
        FocusableComponent::Button("dashboard_refresh".to_string())
    }

    pub fn dashboard_transactions_table() -> FocusableComponent {
        FocusableComponent::Table("dashboard_transactions".to_string())
    }

    // Swap screen components
    pub fn swap_from_asset_input() -> FocusableComponent {
        FocusableComponent::TextInput("swap_from_asset".to_string())
    }

    pub fn swap_from_asset_dropdown() -> FocusableComponent {
        FocusableComponent::Dropdown("swap_from_asset".to_string())
    }

    pub fn swap_to_asset_dropdown() -> FocusableComponent {
        FocusableComponent::Dropdown("swap_to_asset".to_string())
    }

    /// Dropdown for selecting the pool used for the swap
    pub fn swap_pool_dropdown() -> FocusableComponent {
        FocusableComponent::Dropdown("swap_pool".to_string())
    }

    pub fn swap_amount_input() -> FocusableComponent {
        FocusableComponent::TextInput("swap_amount".to_string())
    }

    pub fn swap_slippage_input() -> FocusableComponent {
        FocusableComponent::TextInput("swap_slippage".to_string())
    }

    pub fn swap_execute_button() -> FocusableComponent {
        FocusableComponent::Button("swap_execute".to_string())
    }

    // Pool screen components
    pub fn pools_table() -> FocusableComponent {
        FocusableComponent::Table("pools_list".to_string())
    }

    pub fn pools_search_input() -> FocusableComponent {
        FocusableComponent::TextInput("pools_search".to_string())
    }

    // Liquidity screen components
    pub fn liquidity_pool_dropdown() -> FocusableComponent {
        FocusableComponent::Dropdown("liquidity_pool".to_string())
    }

    pub fn liquidity_amount1_input() -> FocusableComponent {
        FocusableComponent::TextInput("liquidity_amount1".to_string())
    }

    pub fn liquidity_amount2_input() -> FocusableComponent {
        FocusableComponent::TextInput("liquidity_amount2".to_string())
    }

    pub fn liquidity_slippage_input() -> FocusableComponent {
        FocusableComponent::TextInput("liquidity_slippage_amount".to_string())
    }

    pub fn liquidity_provide_button() -> FocusableComponent {
        FocusableComponent::Button("liquidity_provide".to_string())
    }

    pub fn liquidity_withdraw_button() -> FocusableComponent {
        FocusableComponent::Button("liquidity_withdraw".to_string())
    }

    // Rewards screen components
    pub fn rewards_claim_all_button() -> FocusableComponent {
        FocusableComponent::Button("rewards_claim_all".to_string())
    }

    pub fn rewards_epoch_input() -> FocusableComponent {
        FocusableComponent::TextInput("rewards_epoch".to_string())
    }

    pub fn rewards_history_table() -> FocusableComponent {
        FocusableComponent::Table("rewards_history".to_string())
    }

    // Admin screen components
    pub fn admin_create_pool_button() -> FocusableComponent {
        FocusableComponent::Button("admin_create_pool".to_string())
    }

    pub fn admin_asset1_input() -> FocusableComponent {
        FocusableComponent::TextInput("admin_asset1".to_string())
    }

    pub fn admin_asset2_input() -> FocusableComponent {
        FocusableComponent::TextInput("admin_asset2".to_string())
    }

    pub fn admin_fee_input() -> FocusableComponent {
        FocusableComponent::TextInput("admin_fee".to_string())
    }

    // Settings screen components
    pub fn settings_network_dropdown() -> FocusableComponent {
        FocusableComponent::Dropdown("settings_network".to_string())
    }

    pub fn settings_rpc_input() -> FocusableComponent {
        FocusableComponent::TextInput("settings_rpc".to_string())
    }

    pub fn settings_wallet_input() -> FocusableComponent {
        FocusableComponent::TextInput("settings_wallet".to_string())
    }

    pub fn settings_save_button() -> FocusableComponent {
        FocusableComponent::Button("settings_save".to_string())
    }

    pub fn settings_reset_button() -> FocusableComponent {
        FocusableComponent::Button("settings_reset".to_string())
    }

    // Modal components
    pub fn modal_confirm_button() -> FocusableComponent {
        FocusableComponent::Button("modal_confirm".to_string())
    }

    pub fn modal_cancel_button() -> FocusableComponent {
        FocusableComponent::Button("modal_cancel".to_string())
    }

    // Navigation
    pub fn tab_bar() -> FocusableComponent {
        FocusableComponent::TabBar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_manager_creation() {
        let manager = FocusManager::new();
        assert_eq!(manager.current_focus(), None);
        assert_eq!(manager.get_focusable_components().len(), 0);
    }

    #[test]
    fn test_tab_order_navigation() {
        let mut manager = FocusManager::new();
        let components = vec![
            FocusableComponent::TextInput("input1".to_string()),
            FocusableComponent::Button("button1".to_string()),
            FocusableComponent::TextInput("input2".to_string()),
        ];

        manager.set_tab_order(components.clone());

        // Test forward navigation
        assert_eq!(manager.focus_next(), Some(components[0].clone()));
        assert_eq!(manager.focus_next(), Some(components[1].clone()));
        assert_eq!(manager.focus_next(), Some(components[2].clone()));

        // Test backward navigation
        assert_eq!(manager.focus_previous(), Some(components[1].clone()));
        assert_eq!(manager.focus_previous(), Some(components[0].clone()));
    }

    #[test]
    fn test_component_visibility() {
        let mut manager = FocusManager::new();
        let components = vec![
            FocusableComponent::TextInput("input1".to_string()),
            FocusableComponent::Button("button1".to_string()),
        ];

        manager.set_tab_order(components.clone());

        // Hide the first component
        manager.set_component_visibility(components[0].clone(), false);

        // Should skip to the second component
        assert_eq!(manager.focus_next(), Some(components[1].clone()));
    }

    #[test]
    fn test_event_handling() {
        let mut manager = FocusManager::new();
        let components = vec![
            FocusableComponent::TextInput("input1".to_string()),
            FocusableComponent::Button("button1".to_string()),
        ];

        manager.set_tab_order(components.clone());

        // Test event handling
        assert_eq!(
            manager.handle_event(&Event::FocusNext),
            Some(components[0].clone())
        );
        assert_eq!(
            manager.handle_event(&Event::FocusNext),
            Some(components[1].clone())
        );
        assert_eq!(
            manager.handle_event(&Event::FocusPrevious),
            Some(components[0].clone())
        );
    }
}
