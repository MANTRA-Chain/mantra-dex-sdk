use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Simple file logger for TUI application
pub struct FileLogger {
    log_file_path: PathBuf,
}

impl FileLogger {
    /// Create a new file logger
    pub fn new() -> Self {
        // Create logs directory in the user's home directory
        let mut log_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        log_path.push(".mantra-dex");

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&log_path) {
            eprintln!("Warning: Could not create log directory: {}", e);
        }

        log_path.push("tui.log");

        Self {
            log_file_path: log_path,
        }
    }

    /// Log an error message with timestamp
    pub fn log_error(&self, message: &str) {
        self.write_log("ERROR", message);
    }

    /// Log a warning message with timestamp
    pub fn log_warning(&self, message: &str) {
        self.write_log("WARN", message);
    }

    /// Log an info message with timestamp
    pub fn log_info(&self, message: &str) {
        self.write_log("INFO", message);
    }

    /// Log a debug message with timestamp
    pub fn log_debug(&self, message: &str) {
        self.write_log("DEBUG", message);
    }

    /// Write a log entry to the file
    fn write_log(&self, level: &str, message: &str) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let log_entry = format!("[{}] {}: {}\n", timestamp, level, message);

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
        {
            if let Err(e) = file.write_all(log_entry.as_bytes()) {
                eprintln!("Warning: Could not write to log file: {}", e);
            }
        } else {
            eprintln!("Warning: Could not open log file: {:?}", self.log_file_path);
        }
    }

    /// Get the path to the log file
    pub fn get_log_path(&self) -> &PathBuf {
        &self.log_file_path
    }

    /// Clear the log file
    pub fn clear_log(&self) {
        if let Err(e) = std::fs::write(&self.log_file_path, "") {
            eprintln!("Warning: Could not clear log file: {}", e);
        }
    }
}

/// Global logger instance
static mut LOGGER: Option<FileLogger> = None;

/// Initialize the global logger
pub fn init_logger() {
    unsafe {
        LOGGER = Some(FileLogger::new());
    }
}

/// Get the global logger instance
fn get_logger() -> Option<&'static FileLogger> {
    unsafe { LOGGER.as_ref() }
}

/// Log an error message
pub fn log_error(message: &str) {
    if let Some(logger) = get_logger() {
        logger.log_error(message);
    }
}

/// Log a warning message
pub fn log_warning(message: &str) {
    if let Some(logger) = get_logger() {
        logger.log_warning(message);
    }
}

/// Log an info message
pub fn log_info(message: &str) {
    if let Some(logger) = get_logger() {
        logger.log_info(message);
    }
}

/// Log a debug message
pub fn log_debug(message: &str) {
    if let Some(logger) = get_logger() {
        logger.log_debug(message);
    }
}

/// Get the path to the log file
pub fn get_log_file_path() -> Option<PathBuf> {
    get_logger().map(|logger| logger.get_log_path().clone())
}

/// Clear the log file
pub fn clear_log() {
    if let Some(logger) = get_logger() {
        logger.clear_log();
    }
}
