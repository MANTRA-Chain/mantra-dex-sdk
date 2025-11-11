//! MANTRA DEX SDK TUI Module
//!
//! This module provides a Terminal User Interface (TUI) for interacting with the MANTRA DEX SDK.
//! It offers a comprehensive interface for all DEX operations including swaps, liquidity management,
//! rewards, and administrative functions.

#[cfg(feature = "tui-dex")]
pub mod app;
#[cfg(feature = "tui-dex")]
pub mod components;
#[cfg(feature = "tui-dex")]
pub mod events;
#[cfg(feature = "tui-dex")]
pub mod screens;
#[cfg(feature = "tui-dex")]
pub mod ui;
#[cfg(feature = "tui-dex")]
pub mod utils;

#[cfg(feature = "tui-dex")]
pub use app::{App, AppState};
#[cfg(feature = "tui-dex")]
pub use events::{Event, EventHandler};
#[cfg(feature = "tui-dex")]
pub use ui::render_ui;

// Note: Terminal management functions are directly defined in this module and automatically exported

#[cfg(feature = "tui-dex")]
use crate::config::MantraNetworkConfig;
#[cfg(feature = "tui-dex")]
use crate::{Error, MantraDexClient};
#[cfg(feature = "tui-dex")]
use crossterm::{
    cursor, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui-dex")]
use ratatui::{backend::CrosstermBackend, Terminal};
#[cfg(feature = "tui-dex")]
use std::io::{self, Stdout};
#[cfg(feature = "tui-dex")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "tui-dex")]
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Global flag to track if terminal cleanup is needed
#[cfg(feature = "tui-dex")]
static TERMINAL_NEEDS_CLEANUP: AtomicBool = AtomicBool::new(false);

/// Initialize the terminal for TUI mode
///
/// Sets up the terminal with alternate screen and raw mode for TUI interaction.
/// Automatically tracks that cleanup will be needed.
#[cfg(feature = "tui-dex")]
pub fn init_terminal() -> Result<TuiTerminal, Error> {
    enable_raw_mode().map_err(Error::Io)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(Error::Io)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(Error::Io)?;

    // Hide cursor for a cleaner interface
    terminal.hide_cursor().map_err(Error::Io)?;

    // Mark that terminal cleanup will be needed
    TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);

    Ok(terminal)
}

/// Restore the terminal to normal mode
///
/// Disables raw mode, leaves alternate screen, and shows cursor.
/// Safe to call multiple times.
#[cfg(feature = "tui-dex")]
pub fn restore_terminal(terminal: &mut TuiTerminal) -> Result<(), Error> {
    // Only restore if cleanup is needed
    if TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst) {
        disable_raw_mode().map_err(Error::Io)?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(Error::Io)?;
        terminal.show_cursor().map_err(Error::Io)?;

        // Mark cleanup as complete
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Emergency terminal cleanup for panic situations
///
/// Performs basic terminal restoration without error handling to ensure
/// terminal state is restored even during panics.
#[cfg(feature = "tui-dex")]
fn emergency_terminal_cleanup() {
    if TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst) {
        // Ignore errors during emergency cleanup
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), cursor::Show);
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }
}

/// Setup panic handler for graceful terminal restoration
///
/// Installs a panic handler that will restore terminal state before
/// displaying panic information.
#[cfg(feature = "tui-dex")]
pub fn setup_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        emergency_terminal_cleanup();
        original_hook(panic_info);
    }));
}

/// Main TUI application entry point
///
/// This is the primary function to run the TUI application. It handles:
/// - Terminal initialization and cleanup
/// - Panic handler setup
/// - Main application loop
/// - Graceful shutdown
///
/// # Arguments
/// * `client` - The MANTRA DEX client instance
/// * `config` - Network configuration
///
/// # Returns
/// * `Ok(())` - If the application exits normally
/// * `Err(Error)` - If there's an initialization or runtime error
#[cfg(feature = "tui-dex")]
pub async fn run_tui(client: MantraDexClient, config: MantraNetworkConfig) -> Result<(), Error> {
    // Setup panic handler for graceful terminal restoration
    setup_panic_handler();

    // Initialize file logger
    utils::logger::init_logger();
    utils::logger::log_info("MANTRA DEX TUI starting up");

    // Log the file path for user reference
    if let Some(log_path) = utils::logger::get_log_file_path() {
        utils::logger::log_info(&format!("Log file location: {}", log_path.display()));
    }

    // Initialize terminal
    let mut terminal = init_terminal().inspect_err(|e| {
        emergency_terminal_cleanup();
    })?;

    // Create application state
    let mut app = App::new(client, config);
    let mut event_handler = EventHandler::new();

    // Initialize background tasks with event communication
    let event_sender = event_handler.get_sender();
    app.initialize_background_tasks(event_sender);

    // Check for saved wallets and set initial screen
    let wallet_storage = crate::wallet::WalletStorage::new()?;
    if wallet_storage.has_saved_wallets()? {
        // Initialize wallet selection screen with available wallets
        if let Err(e) = app.state.wallet_selection_state.initialize() {
            eprintln!("Warning: Failed to load saved wallets: {}", e);
            // Fall back to dashboard if we can't load wallets
            app.set_status("Welcome to MANTRA DEX TUI! Use Tab/Shift+Tab to navigate, Enter to activate, Esc to go back.".to_string());
            app.navigate_to(crate::tui_dex::app::Screen::Dashboard);
        } else {
            // Show wallet selection screen
            app.state.current_screen = crate::tui_dex::app::Screen::WalletSelection;
            app.state.wizard_state.show_wizard = false; // Don't show wizard when wallets exist
            app.set_status("Select a wallet or create a new one. Use ↑/↓ to navigate, Enter to select, N for new, R to recover.".to_string());
        }
    } else {
        // No saved wallets - show wizard for first-time setup
        app.state.wizard_state.show_wizard = true;
        app.set_status("Welcome to MANTRA DEX! Let's set up your wallet.".to_string());
    }

    // Application result for error handling
    let app_result = run_app_loop(&mut terminal, &mut app, &mut event_handler).await;

    // Stop background tasks before cleanup
    app.stop_background_tasks();

    // Always attempt to restore terminal, even if app_result is an error
    if let Err(restore_error) = restore_terminal(&mut terminal) {
        // If we had an app error, prioritize that, otherwise report restore error
        if app_result.is_ok() {
            return Err(restore_error);
        }
        // Log restore error but return the original app error
        eprintln!("Warning: Failed to restore terminal: {}", restore_error);
    }

    app_result
}

/// Internal application loop
///
/// Separated from run_tui to allow better error handling and cleanup.
#[cfg(feature = "tui-dex")]
async fn run_app_loop(
    terminal: &mut TuiTerminal,
    app: &mut App,
    event_handler: &mut EventHandler,
) -> Result<(), Error> {
    // Main application loop
    loop {
        // Render UI
        terminal
            .draw(|frame| {
                if let Err(e) = render_ui(frame, app) {
                    app.set_error(format!("Render error: {}", e));
                }
            })
            .map_err(Error::Io)?;

        // Handle events with timeout to allow for periodic updates
        match tokio::time::timeout(std::time::Duration::from_millis(100), event_handler.next())
            .await
        {
            Ok(Ok(event)) => match app.handle_event(event).await {
                Ok(_event_was_handled) => {
                    // Event was processed successfully
                    // Don't use the return value to determine quit status
                    // The quit status is managed by app.state.should_quit
                }
                Err(e) => {
                    app.set_error(format!("Event handling error: {}", e));
                }
            },
            Ok(Err(e)) => {
                app.set_error(format!("Event error: {}", e));
            }
            Err(_) => {
                // Timeout - continue loop for periodic updates
                // This allows the UI to refresh even without user input
            }
        }

        // Check if application should quit
        if app.state.should_quit {
            break;
        }
    }

    Ok(())
}

/// Quick terminal check
///
/// Verifies that the terminal supports the required features for the TUI.
/// This can be called before initializing the full TUI to provide better
/// error messages.
#[cfg(feature = "tui-dex")]
pub fn check_terminal_support() -> Result<(), Error> {
    // Check if we're running in a terminal
    if !atty::is(atty::Stream::Stdout) {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "TUI requires a terminal (stdout is not a TTY)",
        )));
    }

    // Try to get terminal size
    let (_width, _height) = crossterm::terminal::size().map_err(Error::Io)?;

    // Check minimum terminal size
    if _width < 80 || _height < 24 {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Terminal too small: {}x{} (minimum: 80x24)",
                _width, _height
            ),
        )));
    }

    Ok(())
}

// Simple atty check implementation
#[cfg(feature = "tui-dex")]
mod atty {
    pub enum Stream {
        Stdout,
    }

    pub fn is(_stream: Stream) -> bool {
        // Simple check - if we can get terminal size, we're probably in a terminal
        crossterm::terminal::size().is_ok()
    }
}

#[cfg(test)]
#[cfg(feature = "tui-dex")]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_terminal_needs_cleanup_flag() {
        // Reset flag to initial state
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);

        // Flag should be false initially
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));

        // After marking that cleanup is needed
        TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);
        assert!(TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));

        // Reset for other tests
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_emergency_terminal_cleanup_when_not_needed() {
        // Reset flag
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);

        // Should be safe to call when cleanup not needed
        emergency_terminal_cleanup();

        // Flag should remain false
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));
    }

    #[test]
    fn test_emergency_terminal_cleanup_when_needed() {
        // Set flag to indicate cleanup needed
        TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);

        // Call emergency cleanup
        emergency_terminal_cleanup();

        // Flag should be reset to false
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));
    }

    #[test]
    fn test_panic_handler_setup() {
        // This test just verifies the function doesn't panic
        // In a real scenario, testing panic handlers is complex
        setup_panic_handler();

        // If we get here, the setup succeeded
        assert!(true);
    }

    #[test]
    fn test_check_terminal_support_success() {
        // In most test environments, this should pass
        // as they run in terminals
        if crossterm::terminal::size().is_ok() {
            // We can't actually test terminal support without a real terminal
            // so we'll just verify the function exists and can be called
            let result = check_terminal_support();
            // The result may be Ok or Err depending on test environment
            // We just want to make sure it doesn't panic
            let _ = result;
        }
    }
}
