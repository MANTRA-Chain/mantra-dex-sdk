//! Reusable UI Components
//!
//! This module contains reusable UI components that can be used across
//! different screens in the TUI application.

// Layout components - implemented in Task 3.1
pub mod header;
pub mod modals;
pub mod navigation;
pub mod status_bar;

// Re-export components for easy access
pub use header::*;
pub use modals::*;
pub use navigation::*;
pub use status_bar::*;

// Data display components - implemented in Task 3.2
pub mod charts;
pub mod tables; // Data visualization components - implemented in Task 6.3

// Input components - implemented in Task 3.3
pub mod forms;
pub mod password_input;
pub mod password_prompt;
pub mod simple_list;
pub mod wallet_save_modal;

pub use charts::*;
pub use forms::*;
pub use password_input::*;
pub use password_prompt::*;
pub use simple_list::*;
pub use tables::*;
pub use wallet_save_modal::*;
