mod download;
mod error;
mod http;
mod part;
mod progress;
mod types;
mod utils;

use clap::Parser;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

use download::{download_parts_parallel, single_download};
use error::ProgramError;
use http::probe;
use part::{merge_parts, split_into_parts};
use progress::{format_bytes, style_spinner};
use types::{Args, ProxyMode};
use utils::{build_client, get_filename_from_url, init_tracing};

#[tokio::main]
async fn main() -> Result<(), ProgramError> {
    let args = Args::parse();

    // Initialize tracing with log level control
    init_tracing(args.log_level, args.debug);

    if args.threads == 0 {
        return Err(ProgramError::ArgNotValid(
            "threads must be >= 1".to_string(),
        ));
    }

    // Derive output path
    let output_path = match args.output {
        Some(p) => p,
        None => PathBuf::from(get_filename_from_url(&args.url)),
    };

    info!("Starting download: {}", args.url);
    info!("Output: {:?}", output_path);

    let proxy_mode = if args.proxy.is_some() {
        ProxyMode::Custom
    } else {
        args.proxy_mode
    };
    let client = build_client(&args.user_agent, proxy_mode, args.proxy.as_deref())?;

    // Probe
    let probe_result = probe(&client, &args.url).await?;
    info!(
        "File size: {} (Accept Ranges: {})",
        format_bytes(probe_result.content_length),
        probe_result.accept_ranges
    );

    // Fallback
    if !probe_result.accept_ranges || args.threads == 1 || probe_result.content_length == 0 {
        warn!("Falling back to single download");
        single_download(
            &client,
            &args.url,
            &output_path,
            probe_result.content_length,
        )
        .await?;
        info!("Download completed successfully");
        return Ok(());
    }

    // Multi-part
    let threads = args
        .threads
        .min(probe_result.content_length as usize)
        .max(1);

    let temp_dir = args
        .temp_dir
        .unwrap_or_else(|| output_path.parent().unwrap_or(Path::new(".")).to_path_buf());

    fs::create_dir_all(&temp_dir).await?;

    let parts = split_into_parts(
        probe_result.content_length,
        threads,
        &output_path,
        &temp_dir,
    )?;

    download_parts_parallel(
        client,
        args.url.clone(),
        parts.clone(),
        probe_result.content_length,
        args.retries,
        args.retry_delay,
    )
    .await?;

    // Merge with a spinner
    let pb_merge = indicatif::ProgressBar::new_spinner();
    pb_merge.set_style(style_spinner());
    pb_merge.set_message("Merging parts...");
    pb_merge.enable_steady_tick(std::time::Duration::from_millis(100));

    merge_parts(&output_path, &parts).await?;

    pb_merge.finish_with_message("Merge completed");

    info!("File saved to {:?}", output_path);
    Ok(())
}
