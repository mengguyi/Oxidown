use indicatif::ProgressStyle;
use std::borrow::Cow;

/// Creates a configured progress bar style for downloads.
///
/// Format: `Spinner [Elapsed] [Bar] Bytes/Total (Speed, ETA)`
/// Uses cyan/blue colors for the bar and green for the spinner.
pub fn style_download_bar() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("#>-")
}

/// Creates a spinner style for indeterminate states (e.g., merging files).
///
/// Format: `Spinner Message`
pub fn style_spinner() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("{spinner:.blue} {msg}")
        .unwrap()
}

/// Helper to format bytes into human-readable strings (KB, MB, GB).
///
/// This function is currently not used by the progress bar (which handles formatting internally),
/// but is available for logging purposes.
#[allow(dead_code)]
pub fn format_bytes(bytes: u64) -> Cow<'static, str> {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64).into()
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64).into()
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64).into()
    } else {
        format!("{} B", bytes).into()
    }
}
