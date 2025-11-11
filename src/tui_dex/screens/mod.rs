//! Screen Implementations
//!
//! This module contains full-screen implementations for different views
//! in the TUI application, such as dashboard, pools, swap, etc.

// Re-export screens when they are implemented
pub mod admin;
pub mod dashboard;
pub mod liquidity;
pub mod multihop;
pub mod pools;
pub mod rewards;
pub mod settings;
pub mod swap;
pub mod transaction;
pub mod wallet_selection;
pub mod wizard;

pub use admin::*;
pub use dashboard::*;
pub use liquidity::*;
pub use multihop::*;
pub use pools::*;
pub use rewards::*;
pub use settings::*;
pub use swap::*;
pub use transaction::*;
pub use wallet_selection::*;

// Placeholder - screens will be implemented in future tasks
