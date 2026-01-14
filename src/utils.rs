use reqwest::{
    Client, Proxy,
    header::{HeaderMap, HeaderValue, USER_AGENT},
};
use tracing::{Level, debug};
use tracing_subscriber::FmtSubscriber;

use crate::error::ProgramError;
use crate::types::{LogLevel, ProxyMode};

/// Initializes the tracing subscriber for logging.
///
/// This function configures `tracing_subscriber::FmtSubscriber` to output logs to stderr.
/// It supports two modes:
/// 1. **User Mode (default)**: Clean output without timestamps or module paths.
/// 2. **Debug Mode (`debug_mode = true`)**: Detailed output with timestamps, file paths, and line numbers.
///
/// # Arguments
///
/// * `level` - The desired log level (Off, Error, Warn, Info, Debug, Trace).
/// * `debug_mode` - If true, forces level to at least DEBUG and enables detailed formatting.
pub fn init_tracing(level: LogLevel, debug_mode: bool) {
    let trace_level = if debug_mode {
        if matches!(level, LogLevel::Trace) {
            Level::TRACE
        } else {
            Level::DEBUG
        }
    } else {
        match level {
            LogLevel::Off => return,
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    };

    let builder = FmtSubscriber::builder()
        .with_max_level(trace_level)
        .with_writer(std::io::stderr);

    if debug_mode {
        builder
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .init();
    } else {
        builder
            .with_target(false)
            .without_time()
            .with_level(true)
            .init();
    }
}

/// Builds and configures the HTTP Client.
///
/// Sets up the User-Agent, Proxy settings (auto, off, or custom), and other default headers.
///
/// # Arguments
///
/// * `ua` - User-Agent string.
/// * `proxy_mode` - Proxy configuration mode.
/// * `proxy` - Optional custom proxy URL (only used if `proxy_mode` is `Custom`).
///
/// # Returns
///
/// * `Ok(Client)` - A configured reqwest Client.
/// * `Err(ProgramError)` - If client configuration fails (e.g. invalid proxy URL).
pub fn build_client(
    ua: &str,
    proxy_mode: ProxyMode,
    proxy: Option<&str>,
) -> Result<Client, ProgramError> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_str(ua)?);

    debug!(
        user_agent = %ua,
        proxy_mode = ?proxy_mode,
        proxy = ?proxy,
        "Building HTTP client"
    );

    let mut builder = Client::builder().default_headers(headers);

    match proxy_mode {
        ProxyMode::Auto => {}
        ProxyMode::Off => {
            builder = builder.no_proxy();
            debug!("Proxy disabled");
        }
        ProxyMode::Custom => {
            let proxy_url = proxy.ok_or_else(|| {
                ProgramError::ArgNotValid("proxy-mode custom requires --proxy <URL>".to_string())
            })?;
            builder = builder.no_proxy();
            builder = builder.proxy(Proxy::all(proxy_url)?);
            debug!(proxy = %proxy_url, "Proxy enabled (custom)");
        }
    }

    let client = builder.build()?;

    Ok(client)
}

/// Derives a filename from a URL path.
///
/// Extracts the last segment of the URL path. If the URL ends in a slash or has no
/// path segments, returns "index.html".
///
/// # Examples
///
/// * `http://example.com/file.zip` -> "file.zip"
/// * `http://example.com/dir/` -> "index.html"
pub fn get_filename_from_url(url_str: &str) -> String {
    if let Ok(url) = reqwest::Url::parse(url_str)
        && let Some(mut segments) = url.path_segments()
        && let Some(last) = segments.next_back()
        && !last.is_empty()
    {
        return last.to_string();
    }
    "index.html".to_string()
}
