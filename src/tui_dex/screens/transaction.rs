//! Transaction Details Screen Implementation
//!
//! This module provides transaction viewing capabilities including:
//! - Individual transaction details viewer
//! - Transaction history with filtering
//! - Export functionality for transaction data

use crate::tui_dex::{
    app::{App, TransactionInfo, TransactionStatus},
    components::{
        header::render_header, navigation::render_navigation, status_bar::render_status_bar,
        tables::format_large_number,
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Padding, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Transaction screen state and filtering options
#[derive(Debug, Clone)]
pub struct TransactionState {
    /// Current view mode
    pub view_mode: TransactionViewMode,
    /// Selected transaction for detailed view
    pub selected_transaction: Option<TransactionInfo>,
    /// Filter options
    pub filters: TransactionFilters,
    /// Search input for transaction hash
    pub search_input: String,
    /// Currently selected transaction index in history
    pub selected_index: usize,
    /// Whether the export modal is shown
    pub show_export_modal: bool,
    /// Export format selection
    pub export_format: ExportFormat,
    /// Current input mode for forms
    pub input_mode: TransactionInputMode,
}

/// Transaction view modes
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionViewMode {
    History,
    Details,
    Export,
}

/// Transaction filtering options
#[derive(Debug, Clone)]
pub struct TransactionFilters {
    /// Filter by transaction status
    pub status_filter: Option<TransactionStatus>,
    /// Filter by operation type
    pub operation_filter: Option<String>,
    /// Filter by date range (simple: last N days)
    pub date_filter: DateFilter,
    /// Show only transactions with gas info
    pub has_gas_info: bool,
}

/// Date filtering options
#[derive(Debug, Clone, PartialEq)]
pub enum DateFilter {
    All,
    LastHour,
    LastDay,
    LastWeek,
    LastMonth,
}

/// Export format options
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Json,
    Csv,
    Text,
}

/// Input modes for transaction screen
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionInputMode {
    None,
    Search,
    ExportFilename,
}

impl Default for TransactionState {
    fn default() -> Self {
        Self {
            view_mode: TransactionViewMode::History,
            selected_transaction: None,
            filters: TransactionFilters::default(),
            search_input: String::new(),
            selected_index: 0,
            show_export_modal: false,
            export_format: ExportFormat::Json,
            input_mode: TransactionInputMode::None,
        }
    }
}

impl Default for TransactionFilters {
    fn default() -> Self {
        Self {
            status_filter: None,
            operation_filter: None,
            date_filter: DateFilter::All,
            has_gas_info: false,
        }
    }
}

impl DateFilter {
    fn as_str(&self) -> &'static str {
        match self {
            DateFilter::All => "All Time",
            DateFilter::LastHour => "Last Hour",
            DateFilter::LastDay => "Last Day",
            DateFilter::LastWeek => "Last Week",
            DateFilter::LastMonth => "Last Month",
        }
    }

    fn all_variants() -> Vec<DateFilter> {
        vec![
            DateFilter::All,
            DateFilter::LastHour,
            DateFilter::LastDay,
            DateFilter::LastWeek,
            DateFilter::LastMonth,
        ]
    }
}

impl ExportFormat {
    fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Json => "JSON",
            ExportFormat::Csv => "CSV",
            ExportFormat::Text => "Text",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
            ExportFormat::Text => "txt",
        }
    }

    fn all_variants() -> Vec<ExportFormat> {
        vec![ExportFormat::Json, ExportFormat::Csv, ExportFormat::Text]
    }
}

/// Render the complete transaction details screen
pub fn render_transaction_screen(f: &mut Frame, app: &App, transaction_state: &TransactionState) {
    let size = f.area();

    // Create main layout: header, nav, content, status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header and navigation
    render_header(f, &app.state, chunks[0]);
    render_navigation(f, &app.state, chunks[1]);

    // Render transaction content based on current view mode
    match transaction_state.view_mode {
        TransactionViewMode::History => {
            render_transaction_history(f, chunks[2], app, transaction_state);
        }
        TransactionViewMode::Details => {
            render_transaction_details(f, chunks[2], app, transaction_state);
        }
        TransactionViewMode::Export => {
            render_export_interface(f, chunks[2], app, transaction_state);
        }
    }

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);

    // Render export modal if shown
    if transaction_state.show_export_modal {
        render_export_confirmation_modal(f, transaction_state);
    }
}

/// Render the transaction history view with filtering
fn render_transaction_history(
    f: &mut Frame,
    area: Rect,
    app: &App,
    transaction_state: &TransactionState,
) {
    let filtered_transactions = filter_transactions(
        &app.state.recent_transactions,
        &transaction_state.filters,
        &transaction_state.search_input,
    );

    if filtered_transactions.is_empty() {
        let empty_msg = Paragraph::new("No transactions match the current filters\n\nTry adjusting your filter criteria or clearing filters with 'C'")
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title("Filtered Transactions"),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Hash").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Time").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Gas Used").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = filtered_transactions
        .iter()
        .enumerate()
        .map(|(index, tx)| {
            let hash_display = if tx.hash.len() > 16 {
                format!("{}...{}", &tx.hash[..8], &tx.hash[tx.hash.len() - 8..])
            } else {
                tx.hash.clone()
            };

            let (status_text, status_color) = match tx.status {
                TransactionStatus::Pending => ("Pending", Color::Yellow),
                TransactionStatus::Success => ("Success", Color::Green),
                TransactionStatus::Failed => ("Failed", Color::Red),
                TransactionStatus::Unknown => ("Unknown", Color::Gray),
            };

            let time_display = tx.timestamp.format("%m/%d %H:%M:%S").to_string();

            let gas_display = match (tx.gas_used, tx.gas_wanted) {
                (Some(used), Some(wanted)) => format!(
                    "{}/{}",
                    format_large_number(&used.to_string()),
                    format_large_number(&wanted.to_string())
                ),
                (Some(used), None) => format_large_number(&used.to_string()),
                _ => "N/A".to_string(),
            };

            let row = Row::new(vec![
                Cell::from(hash_display),
                Cell::from(tx.operation_type.clone()),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(time_display),
                Cell::from(gas_display),
            ]);

            // Highlight selected row
            if index == transaction_state.selected_index {
                row.style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(format!(
                "Transaction History ({} total)",
                filtered_transactions.len()
            )),
    );

    f.render_widget(table, area);

    // Instructions
    let instructions_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let instructions = Paragraph::new(
        "↑/↓: Navigate | Enter: View Details | E: Export | /: Search | F: Filter | C: Clear",
    )
    .style(Style::default().fg(Color::Gray));

    f.render_widget(instructions, instructions_area);
}

/// Render detailed view of a selected transaction
fn render_transaction_details(
    f: &mut Frame,
    area: Rect,
    app: &App,
    transaction_state: &TransactionState,
) {
    let transaction = match &transaction_state.selected_transaction {
        Some(tx) => tx,
        None => {
            let error_msg = Paragraph::new(
                "No transaction selected\n\nPress 'Esc' to return to transaction history",
            )
            .style(Style::default().fg(Color::Red))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Transaction Details"),
            )
            .alignment(Alignment::Center);
            f.render_widget(error_msg, area);
            return;
        }
    };

    // Split into main details and extended info
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Main transaction details
    render_transaction_main_details(f, chunks[0], transaction);

    // Extended transaction information (events, logs, etc.)
    render_transaction_extended_info(f, chunks[1], transaction, app);
}

/// Render main transaction details panel
fn render_transaction_main_details(f: &mut Frame, area: Rect, transaction: &TransactionInfo) {
    let (status_text, status_color) = match transaction.status {
        TransactionStatus::Pending => ("PENDING", Color::Yellow),
        TransactionStatus::Success => ("SUCCESS", Color::Green),
        TransactionStatus::Failed => ("FAILED", Color::Red),
        TransactionStatus::Unknown => ("UNKNOWN", Color::Gray),
    };

    let gas_info = match (transaction.gas_used, transaction.gas_wanted) {
        (Some(used), Some(wanted)) => {
            let efficiency = if wanted > 0 {
                (used as f64 / wanted as f64 * 100.0) as u64
            } else {
                0
            };
            format!(
                "Gas Used: {}\nGas Wanted: {}\nEfficiency: {}%",
                format_large_number(&used.to_string()),
                format_large_number(&wanted.to_string()),
                efficiency
            )
        }
        (Some(used), None) => format!("Gas Used: {}", format_large_number(&used.to_string())),
        (None, Some(wanted)) => format!("Gas Wanted: {}", format_large_number(&wanted.to_string())),
        (None, None) => "Gas Information: Not Available".to_string(),
    };

    let details_text = format!(
        "Transaction Hash:\n{}\n\nOperation Type:\n{}\n\nStatus: {}\n\nTimestamp:\n{}\n\n{}",
        transaction.hash,
        transaction.operation_type,
        status_text,
        transaction.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
        gas_info
    );

    let details_paragraph = Paragraph::new(details_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(status_color))
                .title("Transaction Details")
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details_paragraph, area);
}

/// Render extended transaction information (events, logs, etc.)
fn render_transaction_extended_info(
    f: &mut Frame,
    area: Rect,
    transaction: &TransactionInfo,
    app: &App,
) {
    // Split into two columns: events and additional info
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Event logs panel (placeholder - would need actual event data)
    render_transaction_events(f, chunks[0], transaction);

    // Additional information panel
    render_transaction_additional_info(f, chunks[1], transaction, app);
}

/// Render transaction events panel
fn render_transaction_events(f: &mut Frame, area: Rect, transaction: &TransactionInfo) {
    // This is a placeholder - in a real implementation, you would fetch
    // actual event logs from the transaction
    let events_text = match transaction.status {
        TransactionStatus::Success => {
            format!(
                "Event Logs:\n\n• Transaction executed successfully\n• Operation: {}\n• Gas consumed within limits\n\nNote: Detailed event logs would be\nfetched from the blockchain in a\nfull implementation.",
                transaction.operation_type
            )
        }
        TransactionStatus::Failed => {
            "Event Logs:\n\n• Transaction failed\n• Check error message for details\n• Gas may have been consumed\n\nNote: Error details would be\nfetched from the blockchain\nin a full implementation."
                .to_string()
        }
        TransactionStatus::Pending => {
            "Event Logs:\n\n• Transaction is pending\n• Waiting for confirmation\n• Events will be available\n  once confirmed\n\nNote: Monitor transaction status\nfor updates."
                .to_string()
        }
        TransactionStatus::Unknown => {
            "Event Logs:\n\n• Transaction status unknown\n• Unable to fetch event data\n• Try refreshing or check\n  network connection\n\nNote: Event logs unavailable."
                .to_string()
        }
    };

    let events_paragraph = Paragraph::new(events_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title("Event Logs & Messages")
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(events_paragraph, area);
}

/// Render additional transaction information
fn render_transaction_additional_info(
    f: &mut Frame,
    area: Rect,
    transaction: &TransactionInfo,
    app: &App,
) {
    let block_info = app
        .state
        .block_height
        .map(|h| format!("Current Block: {}", h))
        .unwrap_or_else(|| "Block Height: Unknown".to_string());

    let network_info = app
        .state
        .network_info
        .chain_id
        .as_ref()
        .map(|id| format!("Network: {}", id))
        .unwrap_or_else(|| "Network: Unknown".to_string());

    let wallet_info = app
        .state
        .wallet_address
        .as_ref()
        .map(|addr| format!("Wallet: {}...{}", &addr[..8], &addr[addr.len() - 8..]))
        .unwrap_or_else(|| "Wallet: Not Connected".to_string());

    let age = chrono::Utc::now().signed_duration_since(transaction.timestamp);
    let age_display = if age.num_seconds() < 60 {
        format!("{} seconds ago", age.num_seconds())
    } else if age.num_minutes() < 60 {
        format!("{} minutes ago", age.num_minutes())
    } else if age.num_hours() < 24 {
        format!("{} hours ago", age.num_hours())
    } else {
        format!("{} days ago", age.num_days())
    };

    let additional_text = format!(
        "Additional Information:\n\n{}\n\n{}\n\n{}\n\nTransaction Age:\n{}\n\nActions:\n• Press 'B' to go back\n• Press 'E' to export\n• Press 'R' to refresh",
        block_info,
        network_info,
        wallet_info,
        age_display
    );

    let additional_paragraph = Paragraph::new(additional_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Additional Info")
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(additional_paragraph, area);
}

/// Render export interface
fn render_export_interface(
    f: &mut Frame,
    area: Rect,
    app: &App,
    transaction_state: &TransactionState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Export format selection
            Constraint::Length(10), // Preview
            Constraint::Min(0),     // Instructions
        ])
        .split(area);

    // Export format selection
    render_export_format_selection(f, chunks[0], transaction_state);

    // Export preview
    render_export_preview(f, chunks[1], app, transaction_state);

    // Export instructions
    render_export_instructions(f, chunks[2]);
}

/// Render export format selection
fn render_export_format_selection(f: &mut Frame, area: Rect, transaction_state: &TransactionState) {
    let format_items: Vec<ListItem> = ExportFormat::all_variants()
        .iter()
        .map(|format| {
            ListItem::new(Line::from(vec![
                if transaction_state.export_format == *format {
                    Span::styled("● ", Style::default().fg(Color::Green))
                } else {
                    Span::raw("○ ")
                },
                Span::raw(format.as_str()),
                Span::styled(
                    format!(" (.{})", format.extension()),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect();

    let format_list = List::new(format_items)
        .block(
            Block::default()
                .title("Export Format")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(format_list, area);
}

/// Render export preview
fn render_export_preview(
    f: &mut Frame,
    area: Rect,
    app: &App,
    transaction_state: &TransactionState,
) {
    let filtered_transactions = filter_transactions(
        &app.state.recent_transactions,
        &transaction_state.filters,
        &transaction_state.search_input,
    );

    let preview_content = match transaction_state.export_format {
        ExportFormat::Json => generate_json_preview(&filtered_transactions),
        ExportFormat::Csv => generate_csv_preview(&filtered_transactions),
        ExportFormat::Text => generate_text_preview(&filtered_transactions),
    };

    let preview_paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(
                    "Export Preview ({} transactions)",
                    filtered_transactions.len()
                ))
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(preview_paragraph, area);
}

/// Render export instructions
fn render_export_instructions(f: &mut Frame, area: Rect) {
    let instructions_text = "Export Instructions:\n\n• Use ↑/↓ to select export format\n• Press 'Enter' to confirm export\n• Press 'Esc' to cancel\n• Press 'B' to go back to history\n\nNote: Files will be saved to current directory\nwith timestamp in filename.";

    let instructions_paragraph = Paragraph::new(instructions_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Instructions")
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(instructions_paragraph, area);
}

/// Render export confirmation modal
fn render_export_confirmation_modal(f: &mut Frame, transaction_state: &TransactionState) {
    let area = centered_rect(50, 20, f.area());

    f.render_widget(Clear, area);

    let filename = format!(
        "transactions_{}.{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        transaction_state.export_format.extension()
    );

    let content = format!(
        "Export Confirmation\n\nFormat: {}\nFilename: {}\n\nProceed with export?",
        transaction_state.export_format.as_str(),
        filename
    );

    let modal = Paragraph::new(content)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title("Confirm Export")
                .padding(Padding::uniform(1)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(modal, area);

    // Render action buttons
    let button_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 3,
        width: area.width - 4,
        height: 1,
    };

    let buttons = Paragraph::new("Y: Yes, Export  |  N: No, Cancel")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);

    f.render_widget(buttons, button_area);
}

/// Filter transactions based on current filter settings
fn filter_transactions(
    transactions: &[TransactionInfo],
    filters: &TransactionFilters,
    search_input: &str,
) -> Vec<TransactionInfo> {
    let now = chrono::Utc::now();

    transactions
        .iter()
        .filter(|tx| {
            // Search filter
            if !search_input.is_empty()
                && !tx
                    .hash
                    .to_lowercase()
                    .contains(&search_input.to_lowercase())
            {
                return false;
            }

            // Status filter
            if let Some(ref status_filter) = filters.status_filter {
                if tx.status != *status_filter {
                    return false;
                }
            }

            // Operation filter
            if let Some(ref op_filter) = filters.operation_filter {
                if !tx
                    .operation_type
                    .to_lowercase()
                    .contains(&op_filter.to_lowercase())
                {
                    return false;
                }
            }

            // Date filter
            match filters.date_filter {
                DateFilter::All => {}
                DateFilter::LastHour => {
                    if now.signed_duration_since(tx.timestamp).num_hours() > 1 {
                        return false;
                    }
                }
                DateFilter::LastDay => {
                    if now.signed_duration_since(tx.timestamp).num_days() > 1 {
                        return false;
                    }
                }
                DateFilter::LastWeek => {
                    if now.signed_duration_since(tx.timestamp).num_weeks() > 1 {
                        return false;
                    }
                }
                DateFilter::LastMonth => {
                    if now.signed_duration_since(tx.timestamp).num_days() > 30 {
                        return false;
                    }
                }
            }

            // Gas info filter
            if filters.has_gas_info && tx.gas_used.is_none() && tx.gas_wanted.is_none() {
                return false;
            }

            true
        })
        .cloned()
        .collect()
}

/// Generate JSON preview for export
fn generate_json_preview(transactions: &[TransactionInfo]) -> String {
    if transactions.is_empty() {
        return "{\n  \"transactions\": [],\n  \"count\": 0\n}".to_string();
    }

    let first_tx = &transactions[0];
    format!(
        "{{\n  \"transactions\": [\n    {{\n      \"hash\": \"{}\",\n      \"status\": \"{:?}\",\n      \"operation_type\": \"{}\",\n      \"timestamp\": \"{}\",\n      \"gas_used\": {},\n      \"gas_wanted\": {}\n    }},\n    ...\n  ],\n  \"count\": {}\n}}",
        first_tx.hash,
        first_tx.status,
        first_tx.operation_type,
        first_tx.timestamp.to_rfc3339(),
        first_tx.gas_used.map(|g| g.to_string()).unwrap_or_else(|| "null".to_string()),
        first_tx.gas_wanted.map(|g| g.to_string()).unwrap_or_else(|| "null".to_string()),
        transactions.len()
    )
}

/// Generate CSV preview for export
fn generate_csv_preview(transactions: &[TransactionInfo]) -> String {
    if transactions.is_empty() {
        return "hash,status,operation_type,timestamp,gas_used,gas_wanted\n(no transactions to export)".to_string();
    }

    let first_tx = &transactions[0];
    format!(
        "hash,status,operation_type,timestamp,gas_used,gas_wanted\n\"{}\",\"{:?}\",\"{}\",\"{}\",\"{}\",\"{}\"\n...\n({} total transactions)",
        first_tx.hash,
        first_tx.status,
        first_tx.operation_type,
        first_tx.timestamp.to_rfc3339(),
        first_tx.gas_used.map(|g| g.to_string()).unwrap_or_default(),
        first_tx.gas_wanted.map(|g| g.to_string()).unwrap_or_default(),
        transactions.len()
    )
}

/// Generate text preview for export
fn generate_text_preview(transactions: &[TransactionInfo]) -> String {
    if transactions.is_empty() {
        return "MANTRA DEX Transaction Export\n\nNo transactions to export.".to_string();
    }

    let first_tx = &transactions[0];
    format!(
        "MANTRA DEX Transaction Export\nGenerated: {}\n\nTransaction 1 of {}:\n  Hash: {}\n  Status: {:?}\n  Type: {}\n  Time: {}\n  Gas: {}/{}\n\n...",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        transactions.len(),
        first_tx.hash,
        first_tx.status,
        first_tx.operation_type,
        first_tx.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
        first_tx.gas_used.map(|g| g.to_string()).unwrap_or_else(|| "N/A".to_string()),
        first_tx.gas_wanted.map(|g| g.to_string()).unwrap_or_else(|| "N/A".to_string())
    )
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Export transactions to file (placeholder implementation)
pub fn export_transactions(
    transactions: &[TransactionInfo],
    format: &ExportFormat,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // This is a placeholder implementation
    // In a real application, you would write the actual file export logic here

    match format {
        ExportFormat::Json => {
            // Generate JSON and write to file
            crate::tui_dex::utils::logger::log_info(&format!(
                "Would export {} transactions to {} as JSON",
                transactions.len(),
                filename
            ));
        }
        ExportFormat::Csv => {
            // Generate CSV and write to file
            crate::tui_dex::utils::logger::log_info(&format!(
                "Would export {} transactions to {} as CSV",
                transactions.len(),
                filename
            ));
        }
        ExportFormat::Text => {
            // Generate text and write to file
            crate::tui_dex::utils::logger::log_info(&format!(
                "Would export {} transactions to {} as text",
                transactions.len(),
                filename
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_transaction(
        hash: &str,
        status: TransactionStatus,
        op_type: &str,
    ) -> TransactionInfo {
        TransactionInfo {
            hash: hash.to_string(),
            status,
            operation_type: op_type.to_string(),
            timestamp: Utc::now(),
            gas_used: Some(100000),
            gas_wanted: Some(150000),
        }
    }

    #[test]
    fn test_filter_transactions_by_status() {
        let transactions = vec![
            create_test_transaction("hash1", TransactionStatus::Success, "swap"),
            create_test_transaction("hash2", TransactionStatus::Failed, "liquidity"),
            create_test_transaction("hash3", TransactionStatus::Success, "rewards"),
        ];

        let mut filters = TransactionFilters::default();
        filters.status_filter = Some(TransactionStatus::Success);

        let filtered = filter_transactions(&transactions, &filters, "");
        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|tx| tx.status == TransactionStatus::Success));
    }

    #[test]
    fn test_filter_transactions_by_search() {
        let transactions = vec![
            create_test_transaction("abc123", TransactionStatus::Success, "swap"),
            create_test_transaction("def456", TransactionStatus::Success, "liquidity"),
            create_test_transaction("abc789", TransactionStatus::Success, "rewards"),
        ];

        let filters = TransactionFilters::default();
        let filtered = filter_transactions(&transactions, &filters, "abc");

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|tx| tx.hash.contains("abc")));
    }

    #[test]
    fn test_export_format_methods() {
        let format = ExportFormat::Json;
        assert_eq!(format.as_str(), "JSON");
        assert_eq!(format.extension(), "json");

        let all_formats = ExportFormat::all_variants();
        assert_eq!(all_formats.len(), 3);
    }

    #[test]
    fn test_date_filter_methods() {
        let filter = DateFilter::LastDay;
        assert_eq!(filter.as_str(), "Last Day");

        let all_filters = DateFilter::all_variants();
        assert_eq!(all_filters.len(), 5);
    }

    #[test]
    fn test_transaction_state_default() {
        let state = TransactionState::default();
        assert_eq!(state.view_mode, TransactionViewMode::History);
        assert!(state.selected_transaction.is_none());
        assert!(!state.show_export_modal);
        assert_eq!(state.export_format, ExportFormat::Json);
    }
}
