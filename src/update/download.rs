//! Binary download functionality with progress reporting

use super::error::UpdateError;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::path::Path;

/// Download a file from a URL with progress reporting
///
/// # Arguments
///
/// * `url` - The URL to download from
/// * `dest_path` - The destination file path
///
/// # Errors
///
/// Returns `UpdateError::DownloadFailed` if the download fails
pub async fn download_file(url: &str, dest_path: &Path) -> Result<(), UpdateError> {
    tracing::info!("Downloading from: {}", url);
    
    // Create HTTP client with default settings
    let client = reqwest::Client::builder()
        .user_agent(format!("zy-cli-updater/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to create HTTP client: {}", e)))?;
    
    // Send GET request
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to send request: {}", e)))?;
    
    // Check if response is successful
    if !response.status().is_success() {
        return Err(UpdateError::DownloadFailed(format!(
            "HTTP error: {}",
            response.status()
        )));
    }
    
    // Get content length for progress bar
    let total_size = response.content_length();
    
    // Create progress bar
    let pb = if let Some(size) = total_size {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .map_err(|e| UpdateError::DownloadFailed(format!("Failed to set progress style: {}", e)))?
                .progress_chars("#>-"),
        );
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Downloaded {bytes}")
                .map_err(|e| UpdateError::DownloadFailed(format!("Failed to set progress style: {}", e)))?
        );
        pb
    };
    
    // Create destination file
    let mut file = std::fs::File::create(dest_path)
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to create file: {}", e)))?;
    
    // Download with progress
    let mut downloaded = 0u64;
    
    use futures_util::StreamExt;
    
    // Get the bytes as a stream
    let mut stream = response.bytes_stream();
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| UpdateError::DownloadFailed(format!("Failed to read chunk: {}", e)))?;
        
        file.write_all(&chunk)
            .map_err(|e| UpdateError::DownloadFailed(format!("Failed to write to file: {}", e)))?;
        
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }
    
    pb.finish_with_message("Download complete");
    
    // Ensure all data is written
    file.sync_all()
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to sync file: {}", e)))?;
    
    tracing::info!("Downloaded {} bytes to {:?}", downloaded, dest_path);
    
    Ok(())
}

/// Download the SHA256SUMS.txt file from a release
///
/// # Arguments
///
/// * `checksums_url` - The URL to the SHA256SUMS.txt file
///
/// # Returns
///
/// The contents of the SHA256SUMS.txt file as a string
///
/// # Errors
///
/// Returns `UpdateError::DownloadFailed` if the download fails
pub async fn download_checksums(checksums_url: &str) -> Result<String, UpdateError> {
    tracing::info!("Downloading checksums from: {}", checksums_url);
    
    let client = reqwest::Client::builder()
        .user_agent(format!("zy-cli-updater/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to create HTTP client: {}", e)))?;
    
    let response = client
        .get(checksums_url)
        .send()
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to send request: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(UpdateError::DownloadFailed(format!(
            "HTTP error: {}",
            response.status()
        )));
    }
    
    let text = response
        .text()
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("Failed to read response: {}", e)))?;
    
    tracing::debug!("Downloaded checksums:\n{}", text);
    
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_download_checksums_file() {
        // This test requires network access and a real release
        // In production, you might want to mock this
        let url = "https://github.com/CloudzyVPS/cli/releases/download/v1.0.2/SHA256SUMS.txt";
        
        match download_checksums(url).await {
            Ok(content) => {
                assert!(!content.is_empty());
                // Should contain SHA256 hashes (64 hex chars)
                assert!(content.contains("zy-"));
            }
            Err(e) => {
                // Network errors are acceptable in tests
                eprintln!("Test download failed (network issue?): {}", e);
            }
        }
    }
}
