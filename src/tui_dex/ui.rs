//! UI Rendering Logic
//!
//! This module coordinates the rendering of all UI components and handles
//! the main UI layout and screen rendering with responsive design capabilities.

#[cfg(feature = "tui-dex")]
use crate::tui_dex::app::App;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::components::modals::render_modal;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::dashboard::render_dashboard;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::liquidity::render_liquidity;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::multihop::render_multihop;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::pools::render_pools;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::rewards::render_rewards;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::settings::render_settings_screen;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::swap::render_swap;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::utils::responsive::{create_size_warning_popup, LayoutConfig};
#[cfg(feature = "tui-dex")]
use crate::Error;
#[cfg(feature = "tui-dex")]
use ratatui::{prelude::*, widgets::*};

/// Main UI rendering function with responsive layout support
pub fn render_ui(frame: &mut Frame, app: &mut App) -> Result<(), Error> {
    let size = frame.area();

    // Responsive layout config (still used for size warning)
    let layout_config = LayoutConfig::new(size);

    // Show size warning if terminal is too small
    if layout_config.is_too_small() {
        let (popup_area, clear_widget, warning_widget) = create_size_warning_popup(size);
        frame.render_widget(clear_widget, popup_area);
        frame.render_widget(warning_widget, popup_area);
        return Ok(());
    }

    // Check if wizard should be shown (first time setup or no wallet configured)
    if app.state.wizard_state.show_wizard {
        crate::tui_dex::screens::wizard::render_wizard(frame, app);
        return Ok(());
    }

    // Render the active screen. Screens are responsible for drawing header, navigation,
    // content, and status bar, so we simply delegate rendering here.
    match app.state.current_screen {
        crate::tui_dex::app::Screen::WalletSelection => {
            // Render wallet selection screen
            app.state
                .wallet_selection_state
                .render(frame.area(), frame.buffer_mut());
        }
        crate::tui_dex::app::Screen::Dashboard => render_dashboard(frame, app),
        crate::tui_dex::app::Screen::Pools => render_pools(frame, app),
        crate::tui_dex::app::Screen::Swap => render_swap(frame, app),
        crate::tui_dex::app::Screen::MultiHop => render_multihop(frame, app),
        crate::tui_dex::app::Screen::Liquidity => render_liquidity(frame, app),
        crate::tui_dex::app::Screen::Rewards => render_rewards(frame, app),
        crate::tui_dex::app::Screen::Admin => crate::tui_dex::screens::admin::render_admin(frame, app),
        crate::tui_dex::app::Screen::Settings => {
            // Use enhanced settings screen with focus indicators
            crate::tui_dex::screens::settings::render_settings_screen_with_focus(frame, app);
        }
        crate::tui_dex::app::Screen::TransactionDetails => {
            crate::tui_dex::screens::transaction::render_transaction_screen(
                frame,
                app,
                &app.state.transaction_state,
            );
        }
    }

    // Render modal overlay if present
    if let Some(ref modal_state) = app.state.modal_state {
        render_modal(frame, modal_state, size);
    }

    Ok(())
}

/// Render the header with navigation and basic info (responsive)
fn render_header(frame: &mut Frame, area: Rect, app: &App, layout_config: &LayoutConfig) {
    // Adjust header layout based on screen size
    let header_constraints = match layout_config.mode {
        crate::tui_dex::utils::responsive::LayoutMode::Compact => vec![
            Constraint::Length(20), // Logo/Title (compact)
            Constraint::Min(0),     // Navigation tabs
            Constraint::Length(20), // Wallet/Network info (compact)
        ],
        crate::tui_dex::utils::responsive::LayoutMode::Normal => vec![
            Constraint::Length(30), // Logo/Title
            Constraint::Min(0),     // Navigation tabs
            Constraint::Length(25), // Wallet/Network info
        ],
        crate::tui_dex::utils::responsive::LayoutMode::Expanded => vec![
            Constraint::Length(35), // Logo/Title (expanded)
            Constraint::Min(0),     // Navigation tabs
            Constraint::Length(35), // Wallet/Network info (expanded)
        ],
    };

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(header_constraints)
        .split(area);

    // Title/Logo - responsive text
    let title_text = match layout_config.mode {
        crate::tui_dex::utils::responsive::LayoutMode::Compact => "MANTRA DEX",
        _ => "MANTRA DEX SDK",
    };

    let title = Paragraph::new(title_text)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, header_chunks[0]);

    // Navigation tabs - potentially compact in small screens
    let screen_list = if layout_config.mode == crate::tui_dex::utils::responsive::LayoutMode::Compact {
        // Show abbreviated names in compact mode
        vec![
            ("Dash", crate::tui_dex::app::Screen::Dashboard),
            ("Pools", crate::tui_dex::app::Screen::Pools),
            ("Swap", crate::tui_dex::app::Screen::Swap),
            ("Liq", crate::tui_dex::app::Screen::Liquidity),
            ("Rew", crate::tui_dex::app::Screen::Rewards),
            ("Admin", crate::tui_dex::app::Screen::Admin),
            ("Set", crate::tui_dex::app::Screen::Settings),
        ]
    } else {
        crate::tui_dex::app::Screen::all()
            .iter()
            .map(|s| (s.display_name(), *s))
            .collect()
    };

    let tabs: Vec<Line> = screen_list
        .iter()
        .enumerate()
        .map(|(i, (name, _screen))| {
            let style = if i == app.state.current_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(*name, style))
        })
        .collect();

    let tabs_widget = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL).title("Navigation"))
        .select(app.state.current_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));
    frame.render_widget(tabs_widget, header_chunks[1]);

    // Wallet/Network info - responsive formatting
    let wallet_info = if let Some(address) = &app.state.wallet_address {
        match layout_config.mode {
            crate::tui_dex::utils::responsive::LayoutMode::Compact => {
                format!("W: {}", &address[..6])
            }
            _ => {
                format!("Wallet: {}", &address[..8])
            }
        }
    } else {
        match layout_config.mode {
            crate::tui_dex::utils::responsive::LayoutMode::Compact => "No Wallet".to_string(),
            _ => "No Wallet".to_string(),
        }
    };

    let block_info = if let Some(height) = app.state.block_height {
        match layout_config.mode {
            crate::tui_dex::utils::responsive::LayoutMode::Compact => {
                format!("B: {}", height)
            }
            _ => {
                format!("Block: {}", height)
            }
        }
    } else {
        match layout_config.mode {
            crate::tui_dex::utils::responsive::LayoutMode::Compact => "B: -".to_string(),
            _ => "Block: -".to_string(),
        }
    };

    let info_text = if layout_config.mode == crate::tui_dex::utils::responsive::LayoutMode::Compact {
        format!("{} {}", wallet_info, block_info)
    } else {
        format!("{}\n{}", wallet_info, block_info)
    };

    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Network"));
    frame.render_widget(info, header_chunks[2]);
}

/// Render the main content area based on current screen (responsive)
fn render_main_content(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    layout_config: &LayoutConfig,
) -> Result<(), Error> {
    // Create content layout with optional sidebar
    let content_constraints = layout_config.content_layout_constraints();
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(content_constraints)
        .split(area);

    let main_area = content_chunks[0];

    // Render based on current screen
    match app.state.current_screen {
        crate::tui_dex::app::Screen::WalletSelection => {
            // Pass layout config to wallet selection (will need updating)
            crate::tui_dex::screens::wallet_selection::render_wallet_selection(frame, app);
        }
        crate::tui_dex::app::Screen::Dashboard => {
            // Pass layout config to dashboard (will need updating)
            render_dashboard(frame, app);
        }
        crate::tui_dex::app::Screen::Pools => {
            // Pass layout config to pools (will need updating)
            render_pools(frame, app);
        }
        crate::tui_dex::app::Screen::Swap => {
            // Pass layout config to swap (will need updating)
            render_swap(frame, app);
        }
        crate::tui_dex::app::Screen::MultiHop => {
            // Pass layout config to multihop (will need updating)
            render_multihop(frame, app);
        }
        crate::tui_dex::app::Screen::Liquidity => {
            // Pass layout config to liquidity (will need updating)
            render_liquidity(frame, app);
        }
        crate::tui_dex::app::Screen::Rewards => {
            // Pass layout config to rewards (will need updating)
            render_rewards(frame, app);
        }
        crate::tui_dex::app::Screen::Admin => {
            // Pass layout config to admin (will need updating)
            crate::tui_dex::screens::admin::render_admin(frame, app);
        }
        crate::tui_dex::app::Screen::Settings => {
            // Pass layout config to settings (will need updating)
            render_settings_screen(frame, app);
        }
        crate::tui_dex::app::Screen::TransactionDetails => {
            // Pass layout config to transaction (will need updating)
            crate::tui_dex::screens::transaction::render_transaction_screen(
                frame,
                app,
                &app.state.transaction_state,
            );
        }
    }

    // Render sidebar if enabled and there's space
    if layout_config.show_sidebar && content_chunks.len() > 1 {
        render_sidebar(frame, content_chunks[1], app, layout_config);
    }

    Ok(())
}

/// Render the status bar at the bottom (responsive)
fn render_status_bar(frame: &mut Frame, area: Rect, app: &App, layout_config: &LayoutConfig) {
    // Adjust status bar layout based on screen size
    let status_constraints = match layout_config.mode {
        crate::tui_dex::utils::responsive::LayoutMode::Compact => vec![
            Constraint::Min(0), // Status message (full width in compact)
        ],
        _ => vec![
            Constraint::Min(0),     // Status message
            Constraint::Length(40), // Loading state
        ],
    };

    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(status_constraints)
        .split(area);

    // Status message
    let status_text = if let Some(error) = &app.state.error_message {
        match layout_config.mode {
            crate::tui_dex::utils::responsive::LayoutMode::Compact => {
                format!("Err: {}", error)
            }
            _ => {
                format!("Error: {}", error)
            }
        }
    } else if let Some(status) = &app.state.status_message {
        status.clone()
    } else {
        "Ready".to_string()
    };

    let status_color = if app.state.error_message.is_some() {
        Color::Red
    } else if app.state.status_message.is_some() {
        Color::Green
    } else {
        Color::Blue
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status, status_chunks[0]);

    // Loading state (only in normal/expanded mode)
    if status_chunks.len() > 1 {
        let (loading_text, loading_color) = match &app.state.loading_state {
            crate::tui_dex::app::LoadingState::Idle => ("Idle".to_string(), Color::Gray),
            crate::tui_dex::app::LoadingState::Loading {
                message, progress, ..
            } => {
                let text = if let Some(p) = progress {
                    format!("{} ({}%)", message, *p as u16)
                } else {
                    message.clone()
                };
                (text, Color::Yellow)
            }
            crate::tui_dex::app::LoadingState::Success { message, .. } => {
                (message.clone(), Color::Green)
            }
            crate::tui_dex::app::LoadingState::Error { message, .. } => (message.clone(), Color::Red),
        };

        let loading = Paragraph::new(loading_text)
            .style(Style::default().fg(loading_color))
            .block(Block::default().borders(Borders::ALL).title("State"));
        frame.render_widget(loading, status_chunks[1]);
    }

    // Help text - only show in expanded mode
    if matches!(
        layout_config.mode,
        crate::tui_dex::utils::responsive::LayoutMode::Expanded
    ) {
        let help_text = match app.state.navigation_mode {
            crate::tui_dex::app::NavigationMode::ScreenLevel => {
                "TAB MODE: 1-8: Jump to Tab | Tab/Shift+Tab: Navigate | Enter: Enter Content | q/Esc: Exit App"
            }
            crate::tui_dex::app::NavigationMode::WithinScreen => {
                "CONTENT MODE: Tab/Shift+Tab: Focus | Enter: Activate | Esc: Back to Tab Mode | q: Exit App"
            }
        };
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        // Position help text at bottom of status area
        if area.height > 1 {
            let help_area = Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(1),
                width: area.width,
                height: 1,
            };
            frame.render_widget(help, help_area);
        }
    }
}

/// Render sidebar with quick info and actions
fn render_sidebar(frame: &mut Frame, area: Rect, app: &App, _layout_config: &LayoutConfig) {
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Quick stats
            Constraint::Length(8), // Recent transactions
            Constraint::Min(0),    // Additional info
        ])
        .split(area);

    // Quick stats
    let stats_text = format!(
        "Connected: {}\n\nPools: {}\nBalance: {}\nPending: {}",
        if app.state.wallet_address.is_some() {
            "Yes"
        } else {
            "No"
        },
        app.state.pool_cache.len(),
        app.state.balances.len(),
        app.state.pending_operations.len()
    );

    let stats = Paragraph::new(stats_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Quick Stats"));
    frame.render_widget(stats, sidebar_chunks[0]);

    // Recent transactions
    let recent_txs = if app.state.recent_transactions.is_empty() {
        "No recent transactions".to_string()
    } else {
        app.state
            .recent_transactions
            .iter()
            .take(3)
            .map(|tx| format!("• {}", &tx.hash[..8]))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let transactions = Paragraph::new(recent_txs)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Recent Txs"));
    frame.render_widget(transactions, sidebar_chunks[1]);

    // Additional info
    let help_text = match app.state.navigation_mode {
        crate::tui_dex::app::NavigationMode::ScreenLevel => {
            "TAB MODE:\n\n• 1-8: Jump to Screen\n• Tab/Shift+Tab: Navigate\n• Enter: Enter Content\n• q/Esc: Exit App\n• F1: Help"
        }
        crate::tui_dex::app::NavigationMode::WithinScreen => {
            "CONTENT MODE:\n\n• Tab/Shift+Tab: Focus\n• Enter: Activate\n• Esc: Back to Tab Mode\n• Space: Context Action\n• q: Exit App"
        }
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Navigation Help"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(help, sidebar_chunks[2]);
}

// Placeholder functions for different screens - these will be replaced with actual screen implementations

fn render_pools_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Pools Screen\n\nThis will show:\n• Available pools list\n• Pool details\n• TVL and APY information\n• Pool status indicators")
        .block(Block::default().borders(Borders::ALL).title("Pools"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_swap_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Swap Screen\n\nThis will show:\n• Token swap interface\n• Price impact calculation\n• Slippage settings\n• Transaction confirmation")
        .block(Block::default().borders(Borders::ALL).title("Swap"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_multihop_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Multi-hop Swap Screen\n\nThis will show:\n• Swap route builder\n• Route optimization\n• Multi-step transaction flow\n• Total price impact")
        .block(Block::default().borders(Borders::ALL).title("Multi-hop Swap"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_liquidity_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Liquidity Screen\n\nThis will show:\n• Provide liquidity interface\n• Withdraw liquidity options\n• Current positions\n• LP token management")
        .block(Block::default().borders(Borders::ALL).title("Liquidity"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_rewards_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Rewards Screen\n\nThis will show:\n• Claimable rewards\n• Reward history\n• Epoch information\n• Claim interface")
        .block(Block::default().borders(Borders::ALL).title("Rewards"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_admin_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Admin Screen\n\nThis will show:\n• Pool management\n• Pool creation wizard\n• Feature toggles\n• Administrative controls")
        .block(Block::default().borders(Borders::ALL).title("Admin"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_settings_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Settings Screen\n\nThis will show:\n• Network configuration\n• Wallet management\n• Display preferences\n• Application settings")
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}
