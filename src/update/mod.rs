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

mod error;
mod version;
mod channel;
mod platform;
mod asset;
mod github;

// Re-export public API
pub use error::UpdateError;
pub use version::Version;
pub use channel::Channel;
#[allow(unused_imports)]
pub use platform::Platform;
#[allow(unused_imports)]
pub use asset::{Asset, parse_asset_name, select_asset_for_platform};
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
    
    let current_version = Version::current();
    tracing::debug!("Current version: {}", current_version);
    
    let client = GitHubClient::new(REPO_OWNER.to_string(), REPO_NAME.to_string());
    let latest_release = match client.get_latest_release(channel).await {
        Ok(release) => release,
        Err(UpdateError::NoReleaseFound(_)) => {
            tracing::info!("No releases found for channel {:?}", channel);
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    
    tracing::debug!("Latest release: {}", latest_release.version);
    
    if latest_release.version.is_newer_than(&current_version) {
        tracing::info!(
            "Update available: {} -> {}",
            current_version,
            latest_release.version
        );
        Ok(Some(latest_release))
    } else {
        tracing::info!("Already on the latest version");
        Ok(None)
    }
}
