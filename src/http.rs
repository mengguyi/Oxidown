use reqwest::{
    header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE},
    {Client, Response},
};
use tracing::{debug, instrument, trace};

use crate::error::ProgramError;
use crate::types::ProbeResult;

/// Probes the server to determine file size and range request support.
///
/// It first attempts a HEAD request. If that fails or returns no Content-Length,
/// it falls back to a GET request for the first byte (bytes=0-0) to inspect
/// the Content-Range header.
///
/// # Arguments
///
/// * `client` - The HTTP client.
/// * `url` - The URL to probe.
///
/// # Returns
///
/// * `Ok(ProbeResult)` containing content length and range support status.
/// * `Err(ProgramError)` if network fails or file size cannot be determined.
#[instrument(skip(client), fields(url = %url))]
pub async fn probe(client: &Client, url: &str) -> Result<ProbeResult, ProgramError> {
    // Prefer HEAD, but some servers misbehave; fallback to GET 0-0
    debug!("Sending HEAD request");
    let head = client.head(url).send().await;

    if let Ok(resp) = head {
        debug!(status = %resp.status(), "HEAD response received");
        trace!("HEAD response headers:");
        for (name, value) in resp.headers().iter() {
            trace!(header_name = %name, header_value = ?value);
        }

        let len = parse_content_length(&resp)?;
        let accept_ranges = resp
            .headers()
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase().contains("bytes"))
            .unwrap_or(false);

        if len > 0 {
            debug!(
                content_length = len,
                accept_ranges = accept_ranges,
                "HEAD successful"
            );
            return Ok(ProbeResult {
                content_length: len,
                accept_ranges,
            });
        }
    }

    // Fallback: GET Range: bytes=0-0, then read Content-Range?
    debug!("HEAD failed or returned 0 length, trying GET with Range: bytes=0-0");
    let resp = client.get(url).header(RANGE, "bytes=0-0").send().await?;

    debug!(status = %resp.status(), "Range GET response received");
    trace!("Range GET response headers:");
    for (name, value) in resp.headers().iter() {
        trace!(header_name = %name, header_value = ?value);
    }

    let accept_ranges = resp.status().as_u16() == 206; // Partial Content indicates range support

    // Content-Length in this response is 1, so we need total length:
    // Many servers include Content-Range: bytes 0-0/12345
    let total = resp
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_total_from_content_range)
        .ok_or_else(|| {
            ProgramError::Other(
                "cannot determine total length (no valid Content-Length/Content-Range)".to_string(),
            )
        })?;

    debug!(
        total_size = total,
        accept_ranges = accept_ranges,
        "Probe completed via Range GET"
    );
    Ok(ProbeResult {
        content_length: total,
        accept_ranges,
    })
}

/// Helper to parse total size from Content-Range header.
///
/// Example inputs: "bytes 0-0/12345", "bytes 0-0/*"
/// Returns `None` if size is unknown ("*") or format is invalid.
fn parse_total_from_content_range(s: &str) -> Option<u64> {
    let slash = s.rfind('/')?;
    let total_str = &s[slash + 1..];
    if total_str == "*" {
        return None;
    }
    total_str.parse().ok()
}

/// Helper to parse Content-Length header from a response.
fn parse_content_length(resp: &Response) -> Result<u64, ProgramError> {
    let len = resp
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(len)
}
