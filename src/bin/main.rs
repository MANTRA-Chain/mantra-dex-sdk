//! MANTRA DEX SDK - Terminal User Interface
//!
//! This is the main entry point for the MANTRA DEX TUI application.
//! It provides a complete terminal-based interface for interacting with
//! the MANTRA DEX on the Mantra Dukong Network.

#[cfg(feature = "tui-dex")]
use clap::Parser;
#[cfg(feature = "tui-dex")]
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui-dex")]
use mantra_dex_sdk::{
    client::MantraDexClient,
    config::MantraNetworkConfig,
    error::Error,
    tui::{
        app::{App, Screen},
        events::EventHandler,
        ui::render_ui,
    },
    wallet::MantraWallet,
};
#[cfg(feature = "tui-dex")]
use ratatui::{backend::CrosstermBackend, Terminal};
#[cfg(feature = "tui-dex")]
use std::{fs, io::stdout, panic, path::PathBuf, time::Duration};
#[cfg(feature = "tui-dex")]
use tokio::{sync::mpsc, time::interval};

#[cfg(feature = "tui-dex")]
#[derive(Parser)]
#[command(name = "mantra-dex-tui")]
#[command(about = "MANTRA DEX SDK - Terminal User Interface")]
#[command(version)]
struct Args {
    /// Network to connect to (mainnet, testnet)
    #[arg(short, long, default_value = "testnet")]
    network: String,

    /// Custom RPC endpoint URL
    #[arg(long)]
    rpc_url: Option<String>,

    /// Path to wallet configuration file
    #[arg(short, long)]
    wallet_config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Disable real-time updates
    #[arg(long)]
    no_realtime: bool,

    /// Custom refresh interval in seconds (default: 30)
    #[arg(long, default_value = "30")]
    refresh_interval: u64,
}

#[cfg(feature = "tui-dex")]
#[derive(serde::Deserialize)]
struct WalletConfig {
    mnemonic: String,
    derivation_path: Option<u32>,
    passphrase: Option<String>,
}

#[cfg(feature = "tui-dex")]
async fn load_wallet_from_config(config_path: Option<PathBuf>) -> Result<MantraWallet, Error> {
    let config_path = config_path.unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mantra-dex")
            .join("wallet.toml")
    });

    if !config_path.exists() {
        eprintln!(
            "Wallet configuration file not found at: {}",
            config_path.display()
        );
        eprintln!("Please create a wallet.toml file with your mnemonic:");
        eprintln!("mnemonic = \"your twelve word mnemonic phrase here\"");
        eprintln!("derivation_path = 0  # optional, defaults to 0");
        return Err(Error::Wallet(format!(
            "Wallet config file not found: {}",
            config_path.display()
        )));
    }

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| Error::Wallet(format!("Failed to read wallet config: {}", e)))?;

    let wallet_config: WalletConfig = toml::from_str(&config_content)
        .map_err(|e| Error::Wallet(format!("Failed to parse wallet config: {}", e)))?;

    let derivation_path = wallet_config.derivation_path.unwrap_or(0);
    MantraWallet::from_mnemonic(&wallet_config.mnemonic, derivation_path)
}

#[cfg(feature = "tui-dex")]
async fn setup_client_and_wallet(args: &Args) -> Result<(MantraDexClient, ()), Error> {
    // Setup network configuration
    let mut config = match args.network.as_str() {
        "mainnet" | "testnet" => MantraNetworkConfig::default(),
        _ => {
            return Err(Error::Config(format!(
                "Invalid network: {}. Use 'mainnet' or 'testnet'",
                args.network
            )));
        }
    };

    // Override RPC URL if provided
    if let Some(rpc_url) = &args.rpc_url {
        config.rpc_url = rpc_url.clone();
    }

    // Create client
    let client = MantraDexClient::new(config).await?;

    // Load wallet
    let wallet = load_wallet_from_config(args.wallet_config.clone()).await?;
    let wallet_address = wallet.address()?;
    let client = client.with_wallet(wallet);

    println!("✓ Connected to {} network", args.network);
    println!("✓ Wallet address: {}", wallet_address);

    Ok((client, ()))
}

#[cfg(feature = "tui-dex")]
async fn run_tui_app(args: Args) -> Result<(), Error> {
    // Setup logging if debug enabled
    if args.debug {
        env_logger::init();
    }

    // Setup client and wallet
    let (client, _) = setup_client_and_wallet(&args).await?;

    // Setup panic handler for graceful terminal restoration
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // Create application
    let config = client.config().clone();
    let mut app = App::new(client, config);

    // Extract wallet address from client and set it in app state
    if let Ok(wallet) = app.client.wallet() {
        if let Ok(address) = wallet.address() {
            app.set_wallet_address(address.to_string());
        }
    }

    app.initialize_background_tasks(event_tx.clone());

    // Configure sync settings
    if !args.no_realtime {
        let sync_config = mantra_dex_sdk::tui::utils::async_ops::SyncConfig {
            balance_refresh_interval: Duration::from_secs(args.refresh_interval),
            pool_data_refresh_interval: Duration::from_secs(args.refresh_interval * 2),
            transaction_status_interval: Duration::from_secs(10),
            network_info_interval: Duration::from_secs(60),
            price_update_interval: Duration::from_secs(30),
            network_timeout: Duration::from_secs(10),
            retry_attempts: 3,
            retry_delay: Duration::from_secs(5),
        };
        app.update_sync_config(sync_config);
    }

    // Setup event handler
    let event_handler = EventHandler::new();

    // Check for saved wallets and set initial screen
    let wallet_storage = mantra_dex_sdk::wallet::WalletStorage::new()?;
    if wallet_storage.has_saved_wallets()? {
        // Initialize wallet selection screen with available wallets
        if let Err(e) = app.state.wallet_selection_state.initialize() {
            eprintln!("Warning: Failed to load saved wallets: {}", e);
            // Fall back to dashboard if we can't load wallets
            app.set_status("Welcome to MANTRA DEX TUI! Use Tab/Shift+Tab to navigate, Enter to activate, Esc to go back.".to_string());
            app.navigate_to(Screen::Dashboard);
        } else {
            // Show wallet selection screen
            app.state.current_screen = Screen::WalletSelection;
            app.state.wizard_state.show_wizard = false; // Don't show wizard when wallets exist
            app.set_status("Select a wallet or create a new one. Use ↑/↓ to navigate, Enter to select, N for new, R to recover.".to_string());
        }
    } else {
        // No saved wallets - show wizard for first-time setup
        app.state.wizard_state.show_wizard = true;
        app.set_status("Welcome to MANTRA DEX! Let's set up your wallet.".to_string());
    }

    // Main application loop
    let mut tick_interval = interval(Duration::from_millis(250));

    loop {
        // Render UI
        terminal.draw(|f| {
            if let Err(e) = render_ui(f, &mut app) {
                app.set_error(format!("Render error: {}", e));
            }
        })?;

        // Handle events
        tokio::select! {
            // Handle terminal events
            _ = tick_interval.tick() => {
                if let Ok(crossterm_event) = event::poll(Duration::from_millis(0)) {
                    if crossterm_event {
                        if let Ok(event) = event::read() {
                            if let Some(app_event) = event_handler.handle_crossterm_event(event) {
                                if let Err(e) = event_tx.send(app_event) {
                                    eprintln!("Failed to send event: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            // Handle application events
            Some(event) = event_rx.recv() => {
                if let Err(e) = app.handle_event(event).await {
                    app.set_error(format!("Error handling event: {}", e));
                }

                // Check if we should quit
                if app.state.should_quit {
                    break;
                }
            }

            // Handle graceful shutdown on Ctrl+C
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    // Cleanup
    app.stop_background_tasks();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("Thank you for using MANTRA DEX TUI!");
    Ok(())
}

#[cfg(feature = "tui-dex")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Check if TUI feature is enabled
    if let Err(e) = run_tui_app(args).await {
        eprintln!("TUI Application Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(not(feature = "tui-dex"))]
fn main() {
    eprintln!("TUI feature is not enabled. Please run with: cargo run --features tui");
    std::process::exit(1);
}
