use clap::{Arg, Command};
use mantra_sdk::mcp::{
    logging::{setup_logging, LoggingConfig},
    server::{create_http_server, create_stdio_server, McpServerConfig},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration early to determine logging setup
    let matches = Command::new("Mantra DEX MCP Server")
        .version("0.1.0")
        .about("Model Context Protocol server for Mantra DEX operations")
        .arg(
            Arg::new("transport")
                .long("transport")
                .short('t')
                .value_name("TYPE")
                .help("Transport type: stdio or http")
                .default_value("stdio")
                .value_parser(["stdio", "http"]),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Host to bind HTTP server to")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .value_name("PORT")
                .help("Port to bind HTTP server to")
                .default_value("8080")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("network")
                .long("network")
                .short('n')
                .value_name("NETWORK")
                .help("Network to connect to")
                .default_value("mantra-dukong")
                .value_parser(["mantra-dukong"]),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Enable debug logging")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("log-format")
                .long("log-format")
                .value_name("FORMAT")
                .help("Log format: json, compact, or pretty")
                .default_value("compact")
                .value_parser(["json", "compact", "pretty"]),
        )
        .arg(
            Arg::new("log-file")
                .long("log-file")
                .value_name("FILE")
                .help("Log to file instead of stderr"),
        )
        .arg(
            Arg::new("disable-colors")
                .long("disable-colors")
                .help("Disable colored output")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Parse arguments
    let transport = matches.get_one::<String>("transport").unwrap();
    let host = matches.get_one::<String>("host").unwrap().clone();
    let port = *matches.get_one::<u16>("port").unwrap();
    let network = matches.get_one::<String>("network").unwrap();
    let debug_mode = matches.get_flag("debug");
    let log_format = matches.get_one::<String>("log-format").unwrap();
    let log_file = matches.get_one::<String>("log-file");
    let disable_colors = matches.get_flag("disable-colors");

    // Set up comprehensive logging infrastructure
    let mut logging_config = LoggingConfig::default();

    // Override with CLI arguments
    if debug_mode {
        logging_config.level = mantra_sdk::mcp::logging::LogLevel::Debug;
    } else {
        // For normal operation, use INFO level to reduce noise
        logging_config.level = mantra_sdk::mcp::logging::LogLevel::Info;
    }

    logging_config.format = log_format
        .parse()
        .unwrap_or(mantra_sdk::mcp::logging::LogFormat::Compact);
    logging_config.enable_colors = !disable_colors;

    if let Some(file_path) = log_file {
        logging_config.output_target = mantra_sdk::mcp::logging::LogTarget::File;
        logging_config.log_file_path = Some(std::path::PathBuf::from(file_path));
    }

    // Validate and setup logging
    if let Err(e) = logging_config.validate() {
        eprintln!("Invalid logging configuration: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = setup_logging(&logging_config) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    tracing::info!("Mantra DEX MCP Server starting up");
    tracing::debug!("Logging configuration: {:?}", logging_config);

    // Create server config with environment variables and CLI overrides
    let mut config = match McpServerConfig::with_network(network) {
        Ok(config) => {
            tracing::debug!("Config created successfully");
            config
        }
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    tracing::debug!("Config creation completed");

    // Override debug setting from CLI if provided
    if debug_mode {
        config.debug = debug_mode;
    }

    // Override host and port from CLI if using HTTP transport
    if transport == "http" {
        config.http_host = host;
        config.http_port = port;
    }

    // Validate the final configuration
    tracing::debug!("About to validate configuration...");
    if let Err(e) = config.validate() {
        tracing::error!("Invalid configuration: {}", e);
        std::process::exit(1);
    }
    tracing::debug!("Configuration validation passed");

    tracing::info!(
        transport = ?transport,
        network = ?network,
        debug_mode = ?debug_mode,
        "Starting Mantra DEX MCP Server"
    );

    // Log configuration details
    tracing::debug!(
        config = ?config,
        "Server configuration loaded"
    );

    match transport.as_str() {
        "stdio" => {
            tracing::info!("Using stdio transport for MCP communication");
            tracing::debug!("Starting stdio server with config");
            tracing::debug!("About to call create_stdio_server...");

            match create_stdio_server(config).await {
                Ok(_) => {
                    tracing::info!("Stdio server completed successfully");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to start stdio server");
                    tracing::error!("Error details: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "http" => {
            let http_host = config.http_host.clone();
            let http_port = config.http_port;

            tracing::info!(
                host = ?http_host,
                port = ?http_port,
                "Using HTTP transport for MCP communication"
            );

            if let Err(e) = create_http_server(config).await {
                tracing::error!(error = ?e, "Failed to start HTTP server");
                std::process::exit(1);
            }
        }
        _ => {
            tracing::error!(transport = ?transport, "Invalid transport type");
            std::process::exit(1);
        }
    }

    tracing::info!("Mantra DEX MCP Server shutdown complete");
    Ok(())
}
