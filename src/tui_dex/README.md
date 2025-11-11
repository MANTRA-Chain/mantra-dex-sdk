# MANTRA DEX Terminal User Interface (TUI)

A comprehensive Terminal User Interface for the MANTRA DEX SDK, providing a full-featured, interactive command-line interface for all DEX operations. Built with Rust using `ratatui` and `crossterm` for cross-platform terminal support.

## Architecture Overview

### Core Components

The TUI is organized into several key modules in `src/tui/`:

- **`app.rs`** (2,680 lines) - Central application state management and navigation
- **`events.rs`** (929 lines) - Event handling, keyboard input, and async operations
- **`ui.rs`** - Main UI rendering and layout management
- **`screens/`** - Individual screen implementations for different functionalities
- **`components/`** - Reusable UI components and widgets
- **`utils/`** - Utility functions for focus, formatting, validation, and logging

### Design Philosophy

The TUI follows a clean architectural pattern:

1. **State Management** (`app.rs`) - Centralized state with screen-specific substates
2. **Event-Driven** (`events.rs`) - Async event handling with proper error management
3. **Component-Based** (`components/`) - Reusable widgets and modular UI elements
4. **Screen-Focused** (`screens/`) - Each major functionality gets its own screen module
5. **Responsive Design** (`utils/responsive.rs`) - Adapts to different terminal sizes

## Development Setup

### Prerequisites

```bash
# Rust toolchain (1.70+)
rustup update stable

# Terminal requirements
# - Minimum size: 80x24 characters
# - UTF-8 support recommended
# - Color support recommended (256 colors or more)
```

### Build Commands

```bash
# Development build
cargo build --features tui

# Release build
cargo build --release --features tui

# Run the TUI
cargo run --bin mantra-dex-tui --features tui

# Alternative entry point
cargo run --bin tui --features tui
```

## Code Structure

### Application State Management

The central `App` struct manages all application state:

```rust
pub struct App {
    pub client: MantraDexClient,
    pub config: MantraNetworkConfig,
    pub state: AppState,
    // ... other fields
}

pub struct AppState {
    pub current_screen: Screen,
    pub navigation_mode: NavigationMode,
    pub focus_manager: FocusManager,
    pub dashboard_state: DashboardState,
    pub swap_state: SwapState,
    pub liquidity_state: LiquidityState,
    // ... screen-specific states
}
```

### Screen Navigation

Screens are defined as an enum and managed centrally:

```rust
pub enum Screen {
    WalletSelection,
    Dashboard,
    Pools,
    Swap,
    MultiHop,
    Liquidity,
    Rewards,
    Admin,
    Settings,
    TransactionDetails,
}
```

### Event Handling

Events flow through a centralized handler:

```rust
pub enum Event {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
    Tick,
    BackgroundTask(BackgroundTaskResult),
}

impl App {
    pub async fn handle_event(&mut self, event: Event) -> Result<bool, Error> {
        match event {
            Event::Key(key) => self.handle_key_event(key).await,
            Event::BackgroundTask(result) => self.handle_background_task(result),
            // ... other event types
        }
    }
}
```

### Focus Management

Unified focus management across all screens and components:

```rust
pub struct FocusManager {
    current_focus: usize,
    focusable_items: Vec<String>,
    // ... focus state
}

impl FocusManager {
    pub fn next(&mut self) -> bool { /* Tab navigation */ }
    pub fn previous(&mut self) -> bool { /* Shift+Tab navigation */ }
    pub fn set_focus(&mut self, item: &str) -> bool { /* Direct focus */ }
}
```

## Available Screens

### Core Screens

#### Dashboard
- Wallet overview and balance display
- Quick access to major functions
- Recent transaction history
- Network status and connectivity

#### Wallet Selection
- List saved wallets
- Create new HD wallets
- Import from mnemonic
- Switch between wallets

#### Swap
- Token-to-token swapping interface
- Real-time price simulation
- Slippage configuration
- Transaction confirmation

#### MultiHop
- Multi-hop swapping through multiple pools
- Route optimization display
- Advanced trading features
- Path visualization

#### Liquidity
- Provide liquidity to pools
- Withdraw existing positions
- LP token management
- Impermanent loss calculations

#### Pools
- Browse all available pools
- Pool statistics and information
- Search and filter functionality
- Pool health monitoring

#### Rewards
- View pending rewards
- Claim accumulated rewards
- Reward history tracking
- Staking information

#### Admin (Advanced)
- Pool creation (admin users)
- Pool management functions
- Network administration
- Advanced settings

#### Settings
- Network configuration
- Display preferences
- Logging settings
- Wallet management

### Supporting Screens

#### Transaction Details
- Detailed transaction information
- Status tracking and monitoring
- Error diagnosis and troubleshooting
- Transaction history

## Components Architecture

### Reusable Components

Located in `src/tui/components/`:

- **`charts.rs`** - Price charts and data visualization
- **`forms.rs`** - Input forms and validation
- **`header.rs`** - Application header and navigation
- **`modals.rs`** - Dialog boxes and confirmations
- **`navigation.rs`** - Tab navigation and menu systems
- **`status_bar.rs`** - Status messages and progress indicators
- **`tables.rs`** - Data tables with sorting and pagination
- **`password_input.rs`** - Secure password input fields
- **`wallet_save_modal.rs`** - Wallet save/load dialogs

### Component Usage Example

```rust
// Using a reusable table component
use crate::tui::components::tables::render_data_table;

// In screen render function
let pool_rows: Vec<Row> = pools.iter().map(|pool| {
    Row::new(vec![
        Cell::from(pool.id.to_string()),
        Cell::from(pool.assets[0].denom.clone()),
        Cell::from(pool.assets[1].denom.clone()),
        Cell::from(format_balance(&pool.total_liquidity)),
    ])
}).collect();

render_data_table(
    frame,
    area,
    "Available Pools",
    &["ID", "Asset 1", "Asset 2", "Liquidity"],
    pool_rows,
    selected_index,
    is_focused,
);
```

## Development Workflow

### Adding New Screens

1. **Create screen module** in `src/tui/screens/`:
```rust
// src/tui/screens/my_screen.rs
use crate::tui::app::{App, AppState};
use ratatui::{Frame, layout::Rect};

pub struct MyScreenState {
    pub selected_index: usize,
    pub data: Vec<MyData>,
    // ... screen-specific state
}

impl MyScreenState {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            data: Vec::new(),
        }
    }
}

pub fn render_my_screen(frame: &mut Frame, app: &mut App, area: Rect) -> Result<(), crate::Error> {
    // Implement screen rendering
    Ok(())
}

pub fn handle_my_screen_key(app: &mut App, key: crossterm::event::KeyEvent) -> Result<bool, crate::Error> {
    // Handle screen-specific key events
    Ok(false)
}
```

2. **Add to main screens enum** in `app.rs`:
```rust
pub enum Screen {
    // ... existing screens
    MyScreen,
}
```

3. **Add state to AppState**:
```rust
pub struct AppState {
    // ... existing states
    pub my_screen_state: MyScreenState,
}
```

4. **Register in UI router** (`ui.rs`) and event handler (`app.rs`).

### Adding New Components

1. **Create component module** in `src/tui/components/`:
```rust
// src/tui/components/my_component.rs
use ratatui::{Frame, layout::Rect, widgets::*};

pub fn render_my_component(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    data: &[MyData],
    selected: Option<usize>,
    is_focused: bool,
) -> Result<(), crate::Error> {
    // Component implementation
    Ok(())
}
```

2. **Add to component exports** in `components/mod.rs`.

3. **Use in screens** by importing and calling the render function.

### Adding Background Tasks

For async operations that shouldn't block the UI:

```rust
// In app.rs
pub fn spawn_background_task<F, Fut>(&self, task: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = BackgroundTaskResult> + Send,
{
    let sender = self.background_task_sender.clone();
    tokio::spawn(async move {
        let result = task().await;
        let _ = sender.send(Event::BackgroundTask(result)).await;
    });
}
```

## Key Features

### Keyboard Navigation
- **Tab/Shift+Tab** - Navigate between focusable elements
- **Enter** - Activate selected item
- **Escape** - Go back or cancel
- **Arrow Keys** - Navigate within lists and tables
- **Page Up/Page Down** - Navigate large lists
- **Home/End** - Jump to beginning/end

### Input Handling
- **Text Input** - Full UTF-8 support with validation
- **Numeric Input** - Amount validation with decimal support
- **Password Input** - Secure, masked password entry
- **Address Input** - Bech32 address validation
- **Search/Filter** - Real-time filtering and search

### Visual Features
- **Color Coding** - Status indicators and semantic coloring
- **Progress Bars** - Transaction progress and loading states
- **Charts** - Price history and portfolio visualization
- **Responsive Layout** - Adapts to terminal size changes
- **Modal Dialogs** - Confirmations and detailed information

### Error Handling
- **User-Friendly Messages** - Clear error descriptions
- **Recovery Suggestions** - Actionable error resolution steps
- **Error Logging** - Detailed logs for debugging
- **Graceful Degradation** - Continue operation when possible

## Configuration

### Environment Variables

```bash
# Network configuration
export MANTRA_NETWORK=testnet  # or mainnet, mantra-dukong

# Custom RPC endpoint
export MANTRA_RPC_URL=https://rpc.testnet.mantrachain.io

# Logging settings
export RUST_LOG=info  # debug, info, warn, error
export TUI_LOG_FILE=/tmp/mantra-tui.log  # Custom log file location

# UI preferences
export TUI_THEME=default  # Color theme (future feature)
export TUI_REFRESH_RATE=100  # Milliseconds between UI updates
```

### Terminal Requirements

```bash
# Minimum terminal size
export MIN_TERMINAL_WIDTH=80
export MIN_TERMINAL_HEIGHT=24

# Recommended for best experience
export RECOMMENDED_TERMINAL_WIDTH=120
export RECOMMENDED_TERMINAL_HEIGHT=40
```

## Testing

### Manual Testing Only

The TUI uses manual testing exclusively (no automated UI tests):

```bash
# Test different terminal sizes
resize -s 24 80  # Minimum size
resize -s 40 120 # Recommended size

# Test with different color support
export TERM=xterm-256color
export COLORTERM=truecolor

# Test network switching
MANTRA_NETWORK=testnet cargo run --features tui --bin mantra-dex-tui
MANTRA_NETWORK=mainnet cargo run --features tui --bin mantra-dex-tui
```

### Testing Checklist

1. **Screen Navigation**
   - All screens accessible via navigation
   - Back button functionality
   - Tab navigation works correctly

2. **Input Validation**
   - Invalid amounts rejected
   - Invalid addresses rejected
   - Empty fields handled properly

3. **Error Handling**
   - Network errors displayed clearly
   - Wallet errors handled gracefully
   - Recovery flows work correctly

4. **Responsive Behavior**
   - Terminal resize handled properly
   - Layout adapts to different sizes
   - Scrolling works in all contexts

5. **Async Operations**
   - Background tasks don't block UI
   - Progress indicators work
   - Cancellation handled properly

## Debugging

### Logging

The TUI uses file-based logging to avoid interfering with terminal output:

```bash
# View logs in real-time
tail -f /tmp/mantra-tui.log

# Enable debug logging
RUST_LOG=debug cargo run --features tui --bin mantra-dex-tui

# Custom log file location
TUI_LOG_FILE=/path/to/custom.log cargo run --features tui --bin mantra-dex-tui
```

### Debug Features

```rust
// In utils/logger.rs
pub fn log_debug(message: &str) {
    // Writes to log file, not terminal
    debug!("{}", message);
}

// Usage in screens/components
use crate::tui::utils::logger::log_debug;

log_debug(&format!("User selected pool: {}", pool_id));
```

### Common Debug Scenarios

1. **Event Handling Issues**
   - Check event flow with debug logs
   - Verify focus management state
   - Monitor async task completion

2. **Rendering Problems**
   - Check terminal size constraints
   - Verify layout calculations
   - Monitor frame buffer usage

3. **State Management Issues**
   - Log state transitions
   - Verify screen-specific state updates
   - Check data synchronization

## Performance Considerations

### Efficient Rendering
- Selective screen updates only when needed
- Component-level caching for expensive calculations
- Debounced input handling for rapid key presses

### Memory Management
- State cleanup when switching screens
- Background task cancellation
- Efficient data structures for large lists

### Network Optimization
- Async blockchain operations
- Request batching where possible
- Smart caching of frequently accessed data

## Security

### Sensitive Data Handling
- Private keys never displayed in UI
- Mnemonic phrases shown only during creation/import
- Password input properly masked
- Log files exclude sensitive information

### Input Validation
- All user inputs validated before processing
- Amount validation prevents overflow/underflow
- Address validation using proper bech32 encoding
- Transaction parameter validation

## Contributing

### Development Guidelines

1. **Screen Development**
   - Follow existing screen patterns
   - Implement proper focus management
   - Add comprehensive error handling
   - Use reusable components where possible

2. **Component Development**
   - Make components reusable and configurable
   - Follow consistent naming conventions
   - Implement proper event handling
   - Add inline documentation

3. **Testing**
   - Manual testing across different terminal sizes
   - Test keyboard navigation thoroughly
   - Verify error handling in all scenarios
   - Test with different network conditions

### Code Style

```rust
// Use descriptive function names
pub fn render_wallet_balance_table() -> Result<(), Error> { }

// Group related imports
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    style::{Color, Style},
};

// Document public functions
/// Renders the main swap interface with token selection and amount input
pub fn render_swap_screen(frame: &mut Frame, app: &mut App, area: Rect) -> Result<(), Error> {
    // Implementation
}
```

## Troubleshooting

### Common Issues

1. **Terminal Too Small**
   ```bash
   Error: Terminal too small: 70x20 (minimum: 80x24)
   ```
   Solution: Resize terminal or use `resize -s 30 100`

2. **Color Display Issues**
   ```bash
   export TERM=xterm-256color
   export COLORTERM=truecolor
   ```

3. **Keyboard Navigation Not Working**
   - Check if running in proper terminal
   - Verify terminal supports raw mode
   - Check for conflicting key bindings

4. **Background Tasks Hanging**
   - Check network connectivity
   - Verify RPC endpoint availability
   - Monitor log files for errors

### Recovery Procedures

1. **Terminal State Corrupted**
   ```bash
   # Emergency terminal reset
   reset
   stty sane
   ```

2. **Application Hangs**
   - Ctrl+C should trigger graceful shutdown
   - Terminal state should be automatically restored
   - Check log files for hang location

3. **Wallet Access Issues**
   - Verify wallet files aren't corrupted
   - Check file permissions
   - Try recovering from mnemonic

## Future Enhancements

### Planned Features
- Multi-wallet support improvements
- Advanced charting and analytics
- Customizable keybindings
- Theme system for colors/styling
- Export/import functionality for settings

### Technical Improvements
- Performance optimizations for large datasets
- Better error recovery mechanisms
- Enhanced accessibility features
- Internationalization support
- Advanced network monitoring and diagnostics
