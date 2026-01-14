use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "A blazing fast, multi-threaded file downloader written in Rust."
)]
pub struct Args {
    /// Download URL
    pub url: String,

    /// Output file path (if not provided, derived from URL)
    #[arg(long, short = 'O')]
    pub output: Option<PathBuf>,

    /// Number of concurrent downloads
    #[arg(long, default_value_t = 8)]
    pub threads: usize,

    /// User-Agent to send in every request
    #[arg(long, short = 'A', default_value = "oxidown/0.1.0")]
    pub user_agent: String,

    /// Per-part temp directory (default: same dir as output)
    #[arg(long)]
    pub temp_dir: Option<PathBuf>,

    /// Log level (off, error, warn, info, debug, trace)
    #[arg(long, value_enum, default_value_t = LogLevel::Warn)]
    pub log_level: LogLevel,

    /// Enable debug mode (sets log level to debug and enables detailed output)
    #[arg(long, short = 'v')]
    pub debug: bool,

    /// Max retry attempts per part
    #[arg(long, default_value_t = 50)]
    pub retries: u32,

    /// Initial retry delay in milliseconds
    #[arg(long, default_value_t = 1000)]
    pub retry_delay: u64,

    /// Proxy URL (automatically enables --proxy-mode custom)
    #[arg(long, short = 'x')]
    pub proxy: Option<String>,

    /// Proxy mode: auto (env), off (disable), custom (use --proxy)
    #[arg(long, value_enum, default_value_t = ProxyMode::Auto)]
    pub proxy_mode: ProxyMode,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Represents a download part/chunk
#[derive(Clone, Debug)]
pub struct Part {
    pub idx: usize,
    pub start: u64,
    pub end_inclusive: u64,
    pub path: PathBuf,
}

/// Result of probing server capabilities
pub struct ProbeResult {
    pub content_length: u64,
    pub accept_ranges: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum ProxyMode {
    Auto,
    Off,
    Custom,
}
