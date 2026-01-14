use futures::stream::{self, StreamExt};
use indicatif::ProgressBar;
use reqwest::{Client, header::RANGE};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    time::sleep,
};
use tracing::{debug, error, instrument, warn};

use crate::error::ProgramError;
use crate::progress::style_download_bar;
use crate::types::Part;

/// Performs a single-threaded download for the entire file.
///
/// This function is used as a fallback when the server does not support range requests
/// or when single-threaded download is explicitly requested.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for the request.
/// * `url` - The URL of the file to download.
/// * `output` - The path where the downloaded file should be saved.
/// * `total_size` - The total size of the file in bytes (used for the progress bar).
///
/// # Returns
///
/// * `Ok(())` if the download completes successfully.
/// * `Err(ProgramError)` if an HTTP or I/O error occurs.
#[instrument(skip(client), fields(url = %url, output = ?output))]
pub async fn single_download(
    client: &Client,
    url: &str,
    output: &Path,
    total_size: u64,
) -> Result<(), ProgramError> {
    debug!("Starting single download");

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(ProgramError::Other(format!(
            "GET failed with status {}",
            resp.status()
        )));
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(style_download_bar());
    pb.set_message("Downloading");

    let mut out = File::create(output).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        out.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Download completed");
    Ok(())
}

/// Orchestrates the parallel download of multiple file parts.
///
/// This function manages the concurrent download of file chunks. It initializes a shared
/// progress bar and spawns a background monitor task to update it. It then launches
/// asynchronous tasks for each part, handling retries internally.
///
/// # Arguments
///
/// * `client` - The HTTP client (cloned for each task).
/// * `url` - The URL of the file.
/// * `parts` - A vector of `Part` structs defining the ranges to download.
/// * `total_size` - The total size of the file (for progress display).
/// * `max_retries` - Maximum number of retry attempts per part.
/// * `retry_delay_ms` - Base delay in milliseconds for exponential backoff.
///
/// # Returns
///
/// * `Ok(())` if all parts are downloaded successfully.
/// * `Err(ProgramError)` if any part fails after all retries.
#[instrument(skip(client, parts), fields(url = %url, num_parts = parts.len()))]
pub async fn download_parts_parallel(
    client: Client,
    url: String,
    parts: Vec<Part>,
    total_size: u64,
    max_retries: u32,
    retry_delay_ms: u64,
) -> Result<(), ProgramError> {
    let num_parts = parts.len();

    // Shared state for tracking progress of each part to allow "rewinding" on retry
    let part_progress = Arc::new(
        (0..num_parts)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );

    let pb = ProgressBar::new(total_size);
    pb.set_style(style_download_bar());
    pb.set_message("Downloading parallel");

    // Spawn a background task to update the progress bar from the atomic counters
    let pb_clone = pb.clone();
    let progress_counters = part_progress.clone();
    let monitor_handle = tokio::spawn(async move {
        loop {
            let total_downloaded: u64 = progress_counters
                .iter()
                .map(|a| a.load(Ordering::Relaxed))
                .sum();

            pb_clone.set_position(total_downloaded);

            if total_downloaded >= pb_clone.length().unwrap_or(0)
                && pb_clone.length().unwrap_or(0) > 0
            {
                break;
            }
            if pb_clone.is_finished() {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    });

    let results: Vec<Result<(), ProgramError>> = stream::iter(parts)
        .map(|part| {
            let client = client.clone();
            let url = url.clone();
            let counters = part_progress.clone();
            async move {
                download_one_part_with_retry(
                    &client,
                    &url,
                    &part,
                    &counters,
                    max_retries,
                    retry_delay_ms,
                )
                .await
            }
        })
        .buffer_unordered(num_parts)
        .collect()
        .await;

    // Stop monitor
    monitor_handle.abort();

    // Check results
    for result in results {
        result?;
    }

    pb.finish_with_message("Download completed");
    Ok(())
}

/// Downloads a single part with automatic retries and exponential backoff.
///
/// If a download fails, it waits for a delay (exponentially increasing) and retries.
/// It also resets the progress counter for this part and deletes the partial file
/// before retrying to ensure data integrity.
///
/// # Arguments
///
/// * `client` - The HTTP client.
/// * `url` - The URL.
/// * `part` - The specific part to download.
/// * `counters` - Shared atomic counters for progress tracking.
/// * `max_retries` - Maximum retry attempts.
/// * `retry_delay_ms` - Base retry delay.
#[instrument(skip(client, counters), fields(part = part.idx))]
async fn download_one_part_with_retry(
    client: &Client,
    url: &str,
    part: &Part,
    counters: &[AtomicU64],
    max_retries: u32,
    retry_delay_ms: u64,
) -> Result<(), ProgramError> {
    let mut last_error = ProgramError::Other("no attempts made".to_string());

    for attempt in 1..=max_retries {
        if attempt > 1 {
            debug!(attempt = attempt, "Retrying part download");
            // Reset progress for this part
            counters[part.idx].store(0, Ordering::Relaxed);
            let _ = fs::remove_file(&part.path).await;
        }

        match download_one_part(client, url, part, counters).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_error = e;
                if attempt >= max_retries {
                    break;
                }
                let backoff = retry_delay_ms * 2u64.pow(attempt - 1);

                // Only log warn if it's not the final failure
                warn!(
                    part = part.idx,
                    attempt = attempt,
                    error = %last_error,
                    "Part failed, retrying in {}ms", backoff
                );
                sleep(Duration::from_millis(backoff)).await;
            }
        }
    }

    error!(part = part.idx, "Part failed after all retries");
    Err(last_error)
}

/// Executes the HTTP Range request and streams data to a file for a single part.
///
/// Updates the shared atomic counter as bytes are received.
/// Verifies the final file size against the expected size.
#[instrument(skip(client, counters), fields(part = part.idx))]
async fn download_one_part(
    client: &Client,
    url: &str,
    part: &Part,
    counters: &[AtomicU64],
) -> Result<(), ProgramError> {
    let expected = part.end_inclusive - part.start + 1;

    // Resume check
    if let Ok(meta) = fs::metadata(&part.path).await
        && meta.len() == expected
    {
        debug!(part = part.idx, "Part already complete, skipping");
        counters[part.idx].store(expected, Ordering::Relaxed);
        return Ok(());
    }

    let range = format!("bytes={}-{}", part.start, part.end_inclusive);
    let resp = client.get(url).header(RANGE, range).send().await?;

    if resp.status().as_u16() != 206 {
        return Err(ProgramError::Other(format!(
            "Expected 206 Partial Content, got {}",
            resp.status()
        )));
    }

    let mut file = File::create(&part.path).await?;
    let mut stream = resp.bytes_stream();

    let mut downloaded_so_far = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        file.write_all(&chunk).await?;

        downloaded_so_far += chunk.len() as u64;
        // Update the atomic counter for this part
        counters[part.idx].store(downloaded_so_far, Ordering::Relaxed);
    }
    file.flush().await?;

    let got = fs::metadata(&part.path).await?.len();
    if got != expected {
        return Err(ProgramError::ArgNotValid(format!(
            "Size mismatch: expected {} got {}",
            expected, got
        )));
    }

    Ok(())
}
