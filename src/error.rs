use reqwest::header::InvalidHeaderValue;
use std::fmt;

/// Unified error type for the download program.
///
/// Wraps various error sources (arguments, HTTP, IO) into a single enum
/// for consistent error handling throughout the application.
#[derive(Debug)]
pub enum ProgramError {
    /// Invalid command-line arguments or configuration.
    ArgNotValid(String),
    /// Errors originating from the HTTP client (reqwest).
    Http(reqwest::Error),
    /// File system or network I/O errors.
    Io(std::io::Error),
    /// Generic or miscellaneous errors.
    Other(String),
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::ArgNotValid(msg) => write!(f, "invalid argument: {}", msg),
            ProgramError::Http(e) => write!(f, "HTTP error: {}", e),
            ProgramError::Io(e) => write!(f, "IO error: {}", e),
            ProgramError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl From<InvalidHeaderValue> for ProgramError {
    fn from(msg: InvalidHeaderValue) -> Self {
        ProgramError::ArgNotValid(msg.to_string())
    }
}

impl From<reqwest::Error> for ProgramError {
    fn from(err: reqwest::Error) -> Self {
        ProgramError::Http(err)
    }
}

impl From<std::io::Error> for ProgramError {
    fn from(err: std::io::Error) -> Self {
        ProgramError::Io(err)
    }
}
