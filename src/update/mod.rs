//! Self-update infrastructure for the Zy CLI
//! 
//! This module provides functionality for checking, downloading, and installing
//! updates for the Zy CLI tool from GitHub releases.
//! 
//! # Phase 1: Core Infrastructure
//! 
//! This phase includes:
//! - Version management and comparison
//! - GitHub Releases API integration
//! - Platform detection and asset selection
//! - Release channel management (stable, beta, alpha, rc)
//! - Comprehensive error handling
//! 
//! # Phase 2: Automatic Updates
//! 
//! This phase adds:
//! - Binary download with progress reporting
//! - SHA256 checksum verification
//! - Safe installation with backup and rollback
//! - Platform-specific handling (Unix/Windows)
//! - Complete update flow orchestration
//! 
//! # Examples
//! 
//! Check for updates:
//! 
//! ```no_run
//! use zy::update::{check_for_update, Channel};
//! 
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! if let Some(release) = check_for_update(Channel::Stable).await? {
//!     println!("New version available: {}", release.version);
//! } else {
//!     println!("Already on the latest version");
//! }
//! # Ok(())
//! # }
//! ```
//! 
//! Perform an update:
//! 
//! ```no_run
//! use zy::update::{check_for_update, perform_update, Channel};
//! 
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! if let Some(release) = check_for_update(Channel::Stable).await? {
//!     perform_update(release).await?;
//!     println!("Update complete! Please restart.");
//! }
//! # Ok(())
//! # }
//! ```

mod error;
mod version;
mod channel;
mod platform;
mod asset;
mod github;
pub mod checksum;
mod download;
mod installer;

// Re-export public API
pub use error::UpdateError;
pub use version::Version;
pub use channel::Channel;
pub use platform::Platform;
pub use asset::select_asset_for_platform;
// pub use asset::{Asset, parse_asset_name}; // Preserved for library users
pub use github::{GitHubClient, Release};

/// Repository owner on GitHub
pub const REPO_OWNER: &str = "CloudzyVPS";

/// Repository name on GitHub
pub const REPO_NAME: &str = "cli";

// TODO: Phase 2 - Add Ed25519 public key for verifying release signatures
// pub const RELEASE_PUBLIC_KEY: &[u8] = b"...";

/// Check if a newer version is available for the specified channel
/// 
/// This function compares the current binary version with the latest release
/// available on GitHub for the specified channel.
/// 
/// # Arguments
/// 
/// * `channel` - The release channel to check (Stable, Beta, Alpha, or ReleaseCandidate)
/// 
/// # Returns
/// 
/// - `Ok(Some(Release))` - A newer version is available
/// - `Ok(None)` - Already on the latest version
/// - `Err(UpdateError)` - An error occurred while checking for updates
/// 
/// # Examples
/// 
/// ```no_run
/// use zy::update::{check_for_update, Channel};
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// match check_for_update(Channel::Stable).await {
///     Ok(Some(release)) => {
///         println!("Update available: {} -> {}", 
///             zy::update::Version::current(), 
///             release.version
///         );
///     }
///     Ok(None) => {
///         println!("Up to date!");
///     }
///     Err(e) => {
///         eprintln!("Error checking for updates: {}", e);
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub async fn check_for_update(channel: Channel) -> Result<Option<Release>, UpdateError> {
    tracing::info!("Checking for updates on channel: {:?}", channel);
    println!("Checking for updates on channel: {:?}...", channel);
    
    let current_version = Version::current();
    tracing::debug!("Current version: {}", current_version);
    println!("Current binary version: {}", current_version);
    
    println!("Connecting to GitHub repository: {}/{}...", REPO_OWNER, REPO_NAME);
    let client = GitHubClient::new(REPO_OWNER.to_string(), REPO_NAME.to_string());
    let latest_release = match client.get_latest_release(channel).await {
        Ok(release) => release,
        Err(UpdateError::NoReleaseFound(_)) => {
            tracing::info!("No releases found for channel {:?}", channel);
            println!("No releases found for channel: {:?}", channel);
            return Ok(None);
        }
        Err(e) => {
            tracing::error!(%e, "Failed to fetch latest release");
            println!("Error: {}", e);
            return Err(e);
        }
    };
    
    tracing::debug!("Latest release found: {} (tag: {})", latest_release.version, latest_release.tag_name);
    println!("Latest release found on GitHub: {} (tag: {})", latest_release.version, latest_release.tag_name);
    
    if latest_release.version.is_newer_than(&current_version) {
        tracing::info!(
            "Update available: {} -> {}",
            current_version,
            latest_release.version
        );
        println!("Update available: {} -> {}", current_version, latest_release.version);
        Ok(Some(latest_release))
    } else {
        tracing::info!("Already on the latest version");
        println!("You are already running the latest version.");
        Ok(None)
    }
}

/// Perform a complete update to a new release
/// 
/// This function:
/// 1. Selects the appropriate binary for the current platform
/// 2. Downloads the new binary and checksums
/// 3. Verifies the checksum
/// 4. Creates a backup of the current binary
/// 5. Installs the new binary
/// 6. Cleans up on success or rolls back on failure
/// 
/// # Arguments
/// 
/// * `release` - The release to update to
/// 
/// # Returns
/// 
/// `Ok(())` on successful update, `Err(UpdateError)` on failure
/// 
/// # Errors
/// 
/// Returns various `UpdateError` variants if any step of the update fails.
/// On failure, the function attempts to rollback to the previous binary.
/// 
/// # Examples
/// 
/// ```no_run
/// use zy::update::{check_for_update, perform_update, Channel};
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// if let Some(release) = check_for_update(Channel::Stable).await? {
///     perform_update(release).await?;
///     println!("Update complete! Please restart the application.");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn perform_update(release: Release) -> Result<(), UpdateError> {
    tracing::info!("Starting update to version {}", release.version);
    println!("\n{}", yansi::Paint::new("Starting update process...").bold());
    
    // Step 1: Select the appropriate asset for this platform
    println!("Step 1/5: Selecting binary for your platform...");
    let platform = Platform::current();
    platform.is_supported()?;
    
    let binary_asset = asset::select_asset_for_platform(&release.assets, &platform)?;
    
    println!(
        "  Selected: {} ({} bytes)",
        yansi::Paint::new(&binary_asset.name).cyan(),
        format_bytes(binary_asset.size)
    );
    
    // Step 2: Find and download the SHA256SUMS.txt file
    println!("\nStep 2/5: Downloading checksums...");
    let checksums_asset = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.txt")
        .ok_or(UpdateError::ChecksumFileNotFound)?;
    
    let checksums_content = download::download_checksums(&checksums_asset.download_url).await?;
    let checksums = checksum::parse_checksums(&checksums_content)?;
    
    let expected_hash = checksums
        .get(&binary_asset.name)
        .ok_or_else(|| {
            UpdateError::ChecksumFileNotFound
        })?
        .clone();
    
    println!("  Expected SHA256: {}", yansi::Paint::new(&expected_hash).dim());
    
    // Step 3: Download the new binary
    println!("\nStep 3/5: Downloading new binary...");
    let temp_dir = tempfile::tempdir().map_err(|e| {
        UpdateError::DownloadFailed(format!("Failed to create temp directory: {}", e))
    })?;
    
    let download_path = temp_dir.path().join(&binary_asset.name);
    download::download_file(&binary_asset.download_url, &download_path).await?;
    
    // Step 4: Verify checksum
    println!("\nStep 4/5: Verifying checksum...");
    checksum::verify_file_hash(&download_path, &expected_hash).await?;
    println!("  {}", yansi::Paint::new("✓ Checksum verified successfully").green());
    
    // Step 5: Install the new binary
    println!("\nStep 5/5: Installing new binary...");
    let current_exe = installer::get_current_executable()?;
    
    println!("  Creating backup of current binary...");
    installer::install_binary(&download_path, &current_exe).await?;
    
    println!("\n{}", yansi::Paint::new("✓ Update completed successfully!").green().bold());
    println!("\n{}", yansi::Paint::new("Please restart the application to use the new version.").yellow());
    
    tracing::info!("Update completed successfully");
    
    Ok(())
}

/// Format bytes as a human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
