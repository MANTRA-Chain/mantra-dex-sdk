//! MANTRA DEX SDK - Simple TUI Launcher
//!
//! This is a simplified entry point for the MANTRA DEX TUI application.

#[cfg(feature = "tui-dex")]
use clap::Parser;
#[cfg(feature = "tui-dex")]
use mantra_dex_sdk::{client::MantraDexClient, config::MantraNetworkConfig, tui::run_tui};

#[cfg(feature = "tui-dex")]
#[derive(Parser)]
#[command(name = "mantra-dex")]
#[command(about = "MANTRA DEX Terminal User Interface")]
#[command(version)]
struct Args {
    /// Network to connect to (testnet recommended for testing)
    #[arg(short, long, default_value = "testnet")]
    network: String,

    /// Show help information
    #[arg(long)]
    help_mode: bool,
}

#[cfg(feature = "tui-dex")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.help_mode {
        println!("MANTRA DEX TUI - Terminal User Interface for MANTRA DEX");
        println!();
        println!("REQUIREMENTS:");
        println!("1. Create a wallet configuration file at ~/.mantra-dex/wallet.toml");
        println!("   Example content:");
        println!("   mnemonic = \"your twelve word mnemonic phrase here\"");
        println!("   derivation_path = 0");
        println!();
        println!("2. Ensure you have network connectivity to the Mantra chain");
        println!();
        println!("USAGE:");
        println!("  cargo run --bin tui --features tui");
        println!("  cargo run --bin tui --features tui -- --network testnet");
        println!();
        println!("CONTROLS:");
        println!("  Tab/Shift+Tab - Navigate between screens");
        println!("  Arrow keys    - Navigate within screens");
        println!("  Enter         - Activate/confirm");
        println!("  Esc           - Go back/cancel");
        println!("  q             - Quit application");
        println!("  h             - Show help");
        return Ok(());
    }

    // Create a default configuration and client
    let config = MantraNetworkConfig::default();
    let client = MantraDexClient::new(config.clone()).await?;

    println!("🚀 Starting MANTRA DEX TUI...");
    println!("📁 Make sure your wallet config is at ~/.mantra-dex/wallet.toml");

    // Start the TUI application
    if let Err(e) = run_tui(client, config).await {
        eprintln!("❌ TUI Error: {}", e);
        eprintln!();
        eprintln!("💡 Common issues:");
        eprintln!("  - Missing wallet configuration file");
        eprintln!("  - Network connectivity issues");
        eprintln!("  - Terminal not supported");
        eprintln!();
        eprintln!("Run with --help-mode for setup instructions");
        std::process::exit(1);
    }

    println!("👋 Thank you for using MANTRA DEX TUI!");
    Ok(())
}

#[cfg(not(feature = "tui-dex"))]
fn main() {
    eprintln!("TUI feature is not enabled. Please run with: cargo run --bin tui --features tui");
    std::process::exit(1);
}
