use std::path::Path;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncWriteExt, copy},
};
use tracing::{debug, info, instrument};

use crate::error::ProgramError;
use crate::progress::format_bytes;
use crate::types::Part;

impl Part {
    /// Returns the expected size of this part in bytes
    #[allow(dead_code)]
    pub fn expected_size(&self) -> u64 {
        self.end_inclusive - self.start + 1
    }
}

/// Calculates part boundaries and splits the total file size into equal chunks.
///
/// # Arguments
///
/// * `total_len` - Total size of the file in bytes.
/// * `threads` - Number of parts/threads to split into.
/// * `output` - The final output path (used to name temporary part files).
/// * `temp_dir` - Directory where temporary part files will be stored.
///
/// # Returns
///
/// * `Ok(Vec<Part>)` - A vector of `Part` structs describing each chunk.
pub fn split_into_parts(
    total_len: u64,
    threads: usize,
    output: &Path,
    temp_dir: &Path,
) -> Result<Vec<Part>, ProgramError> {
    let mut parts = Vec::with_capacity(threads);

    let base_name = output
        .file_name()
        .ok_or_else(|| ProgramError::ArgNotValid("output has no file name".to_string()))?
        .to_string_lossy()
        .to_string();

    let chunk = total_len / threads as u64;
    let mut start = 0u64;

    for idx in 0..threads {
        let mut end = if idx == threads - 1 {
            total_len - 1
        } else {
            start + chunk - 1
        };

        // Handle tiny files / rounding
        if end >= total_len {
            end = total_len - 1;
        }

        let part_path = temp_dir.join(format!("{}.part{}", base_name, idx));
        parts.push(Part {
            idx,
            start,
            end_inclusive: end,
            path: part_path,
        });

        start = end + 1;
        if start >= total_len {
            break;
        }
    }

    debug!(
        total_len = total_len,
        threads = threads,
        actual_parts = parts.len(),
        "File split into parts"
    );

    Ok(parts)
}

/// Merges all downloaded parts into the final output file.
///
/// This function reads each temporary part file in order and appends it to the output file.
/// It uses efficient `io::copy` (which may use zero-copy syscalls like `sendfile` on Linux).
/// After successful merging, temporary files are deleted.
///
/// # Arguments
///
/// * `output` - Path to the final output file.
/// * `parts` - Vector of parts (used to locate temp files).
#[instrument(skip(parts), fields(output = ?output, num_parts = parts.len()))]
pub async fn merge_parts(output: &Path, parts: &[Part]) -> Result<(), ProgramError> {
    info!("Merging parts into final file");
    debug!("Merging {} parts into {:?}...", parts.len(), output);

    // Sort by idx to ensure correct order
    let mut parts_sorted = parts.to_vec();
    parts_sorted.sort_by_key(|p| p.idx);

    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output)
        .await?;

    let mut total_merged: u64 = 0;
    for p in &parts_sorted {
        debug!(part = p.idx, path = ?p.path, "Merging part");
        let mut f = File::open(&p.path).await?;
        let copied = copy(&mut f, &mut out).await?;
        total_merged += copied;
        debug!(part = p.idx, bytes = copied, "Part merged");
    }
    out.flush().await?;

    info!(
        total_merged = total_merged,
        total_human = %format_bytes(total_merged),
        "Merge completed"
    );

    // Cleanup
    debug!("Cleaning up temporary part files");
    for p in &parts_sorted {
        debug!(part = p.idx, path = ?p.path, "Removing temp file");
        fs::remove_file(&p.path).await?;
    }

    Ok(())
}
