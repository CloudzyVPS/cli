/// Error types for the update module
use thiserror::Error;

use super::channel::Channel;

/// Errors that can occur during update operations
#[derive(Debug, Error)]
pub enum UpdateError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),
    
    /// GitHub API rate limit exceeded
    #[error("GitHub API rate limit exceeded. Resets at {reset_time}")]
    RateLimitExceeded { 
        /// Time when the rate limit resets (ISO 8601 format)
        reset_time: String 
    },
    
    /// No release found for the specified channel
    #[error("No release found for channel: {0:?}")]
    NoReleaseFound(Channel),
    
    /// No asset found for the current platform
    #[allow(dead_code)]
    #[error("No asset found for platform: {0}")]
    NoAssetFound(String),
    
    /// Invalid semantic version format
    #[error("Invalid version format: {0}")]
    InvalidVersion(String),
    
    /// Platform is not supported for updates
    #[allow(dead_code)]
    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),
    
    /// GitHub API returned an error
    #[error("GitHub API error: {0}")]
    GitHubApiError(String),
    
    /// Failed to download update
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    
    /// Checksum verification failed
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum
        expected: String,
        /// Actual checksum
        actual: String,
    },
    
    /// Checksum file not found in release assets
    #[error("Checksum file not found in release")]
    ChecksumFileNotFound,
    
    /// Failed to install update
    #[error("Installation failed: {0}")]
    InstallationFailed(String),
    
    /// Failed to create backup
    #[error("Backup failed: {0}")]
    BackupFailed(String),
    
    /// Failed to rollback after error
    #[error("Rollback failed: {0}")]
    RollbackFailed(String),
    
    /// Permission denied during update
    #[error("Permission denied: {0}")]
    #[allow(dead_code)]
    PermissionDenied(String),
    
    /// I/O error during update
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
