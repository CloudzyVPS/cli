//! Safe installation with backup and rollback functionality

use super::error::UpdateError;
use std::path::{Path, PathBuf};

/// Get the path to the current executable
///
/// # Errors
///
/// Returns an error if the current executable path cannot be determined
pub fn get_current_executable() -> Result<PathBuf, UpdateError> {
    std::env::current_exe().map_err(|e| {
        UpdateError::InstallationFailed(format!("Failed to get current executable path: {}", e))
    })
}

/// Create a backup of the current binary
///
/// The backup will have a `.bak` extension
///
/// # Arguments
///
/// * `current_path` - Path to the current binary
///
/// # Returns
///
/// The path to the backup file
///
/// # Errors
///
/// Returns `UpdateError::BackupFailed` if the backup cannot be created
pub fn create_backup(current_path: &Path) -> Result<PathBuf, UpdateError> {
    let backup_path = current_path.with_extension("bak");
    
    tracing::info!("Creating backup: {:?} -> {:?}", current_path, backup_path);
    
    // Copy the current binary to backup location
    std::fs::copy(current_path, &backup_path).map_err(|e| {
        UpdateError::BackupFailed(format!(
            "Failed to copy {:?} to {:?}: {}",
            current_path, backup_path, e
        ))
    })?;
    
    // On Unix, preserve executable permissions
    #[cfg(unix)]
    {
        #[allow(unused_imports)]
        use std::os::unix::fs::PermissionsExt;
        
        let metadata = std::fs::metadata(current_path).map_err(|e| {
            UpdateError::BackupFailed(format!("Failed to read permissions: {}", e))
        })?;
        
        let permissions = metadata.permissions();
        std::fs::set_permissions(&backup_path, permissions).map_err(|e| {
            UpdateError::BackupFailed(format!("Failed to set backup permissions: {}", e))
        })?;
    }
    
    tracing::info!("Backup created successfully");
    
    Ok(backup_path)
}

/// Restore from backup (rollback)
///
/// # Arguments
///
/// * `backup_path` - Path to the backup file
/// * `target_path` - Path to restore to
///
/// # Errors
///
/// Returns `UpdateError::RollbackFailed` if the rollback cannot be completed
pub fn restore_from_backup(backup_path: &Path, target_path: &Path) -> Result<(), UpdateError> {
    tracing::warn!("Rolling back: {:?} -> {:?}", backup_path, target_path);
    
    if !backup_path.exists() {
        return Err(UpdateError::RollbackFailed(format!(
            "Backup file does not exist: {:?}",
            backup_path
        )));
    }
    
    // On Unix, use rename for atomic operation
    #[cfg(unix)]
    {
        std::fs::rename(backup_path, target_path).map_err(|e| {
            UpdateError::RollbackFailed(format!(
                "Failed to rename {:?} to {:?}: {}",
                backup_path, target_path, e
            ))
        })?;
    }
    
    // On Windows, we need to copy because the running executable might be locked
    #[cfg(windows)]
    {
        std::fs::copy(backup_path, target_path).map_err(|e| {
            UpdateError::RollbackFailed(format!(
                "Failed to copy {:?} to {:?}: {}",
                backup_path, target_path, e
            ))
        })?;
        
        // Try to delete backup, but don't fail if we can't
        let _ = std::fs::remove_file(backup_path);
    }
    
    tracing::info!("Rollback completed successfully");
    
    Ok(())
}

/// Install a new binary, replacing the current one
///
/// This function:
/// 1. Creates a backup of the current binary
/// 2. Installs the new binary
/// 3. Verifies the installation
/// 4. Cleans up the backup on success
/// 5. Rolls back on failure
///
/// # Arguments
///
/// * `new_binary_path` - Path to the new binary file
/// * `current_path` - Path to the current binary (will be replaced)
///
/// # Errors
///
/// Returns various `UpdateError` variants if installation fails.
/// On failure, attempts to rollback to the backup.
///
/// # Platform-specific behavior
///
/// - **Unix/Linux/macOS**: Uses atomic `rename()` for safe replacement
/// - **Windows**: Handles locked executable files by using copy operations
pub async fn install_binary(new_binary_path: &Path, current_path: &Path) -> Result<(), UpdateError> {
    tracing::info!("Installing new binary: {:?} -> {:?}", new_binary_path, current_path);
    
    // Step 1: Create backup
    let backup_path = create_backup(current_path)?;
    
    // Step 2: Install new binary
    let install_result = install_new_binary(new_binary_path, current_path).await;
    
    match install_result {
        Ok(_) => {
            tracing::info!("New binary installed successfully");
            
            // Step 3: Verify the installation
            if let Err(e) = verify_installation(current_path) {
                tracing::error!("Installation verification failed: {}", e);
                
                // Rollback
                if let Err(rollback_err) = restore_from_backup(&backup_path, current_path) {
                    tracing::error!("Rollback failed: {}", rollback_err);
                    return Err(UpdateError::RollbackFailed(format!(
                        "Installation failed and rollback also failed: {} (original error: {})",
                        rollback_err, e
                    )));
                }
                
                return Err(UpdateError::InstallationFailed(format!(
                    "Verification failed, rolled back: {}",
                    e
                )));
            }
            
            // Step 4: Clean up backup
            if let Err(e) = std::fs::remove_file(&backup_path) {
                tracing::warn!("Failed to remove backup file {:?}: {}", backup_path, e);
                // Non-fatal - we'll just leave the backup there
            } else {
                tracing::info!("Backup file removed");
            }
            
            Ok(())
        }
        Err(e) => {
            tracing::error!("Installation failed: {}", e);
            
            // Rollback
            if let Err(rollback_err) = restore_from_backup(&backup_path, current_path) {
                tracing::error!("Rollback failed: {}", rollback_err);
                return Err(UpdateError::RollbackFailed(format!(
                    "Installation failed and rollback also failed: {} (original error: {})",
                    rollback_err, e
                )));
            }
            
            Err(UpdateError::InstallationFailed(format!(
                "Installation failed, rolled back: {}",
                e
            )))
        }
    }
}

/// Install the new binary (platform-specific implementation)
#[cfg(unix)]
async fn install_new_binary(new_binary_path: &Path, current_path: &Path) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;
    
    // Set executable permissions on the new binary
    let permissions = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(new_binary_path, permissions).map_err(|e| {
        UpdateError::InstallationFailed(format!("Failed to set executable permissions: {}", e))
    })?;
    
    // Use atomic rename for safe replacement
    std::fs::rename(new_binary_path, current_path).map_err(|e| {
        UpdateError::InstallationFailed(format!("Failed to replace binary: {}", e))
    })?;
    
    Ok(())
}

/// Install the new binary (Windows-specific implementation)
#[cfg(windows)]
async fn install_new_binary(new_binary_path: &Path, current_path: &Path) -> Result<(), UpdateError> {
    // On Windows, we can't replace a running executable directly
    // We need to use a different approach:
    // 1. Try to copy the new binary over the old one
    // 2. If that fails due to locking, we'll need to rename the current one first
    
    // Try direct copy first
    match std::fs::copy(new_binary_path, current_path) {
        Ok(_) => {
            tracing::info!("Binary replaced successfully");
            return Ok(());
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!("Direct replacement failed (file locked), trying alternative method");
            
            // Try the rename method
            let temp_old = current_path.with_extension("old");
            
            // Remove old temp file if it exists
            let _ = std::fs::remove_file(&temp_old);
            
            // Rename current to .old
            std::fs::rename(current_path, &temp_old).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to rename old binary: {}", e))
            })?;
            
            // Copy new binary to the target location
            std::fs::copy(new_binary_path, current_path).map_err(|e| {
                // Try to restore the old binary
                let _ = std::fs::rename(&temp_old, current_path);
                UpdateError::InstallationFailed(format!("Failed to copy new binary: {}", e))
            })?;
            
            // Schedule old binary for deletion (best effort)
            let _ = std::fs::remove_file(&temp_old);
            
            Ok(())
        }
        Err(e) => {
            Err(UpdateError::InstallationFailed(format!("Failed to replace binary: {}", e)))
        }
    }
}

/// Verify that the installed binary is valid
fn verify_installation(binary_path: &Path) -> Result<(), UpdateError> {
    // Check that the file exists
    if !binary_path.exists() {
        return Err(UpdateError::InstallationFailed(
            "Binary does not exist after installation".to_string()
        ));
    }
    
    // Check that it's a regular file
    let metadata = std::fs::metadata(binary_path).map_err(|e| {
        UpdateError::InstallationFailed(format!("Failed to read binary metadata: {}", e))
    })?;
    
    if !metadata.is_file() {
        return Err(UpdateError::InstallationFailed(
            "Binary is not a regular file".to_string()
        ));
    }
    
    // Check that it has some reasonable size (at least 100KB)
    if metadata.len() < 100_000 {
        return Err(UpdateError::InstallationFailed(format!(
            "Binary is suspiciously small: {} bytes",
            metadata.len()
        )));
    }
    
    // On Unix, check that it's executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        
        // Check if any execute bit is set
        if mode & 0o111 == 0 {
            return Err(UpdateError::InstallationFailed(
                "Binary is not executable".to_string()
            ));
        }
    }
    
    tracing::info!("Installation verification passed");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    
    #[test]
    fn test_create_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("test-binary");
        
        // Create a dummy binary
        let mut file = std::fs::File::create(&binary_path).unwrap();
        file.write_all(b"test binary content").unwrap();
        drop(file);
        
        // Create backup
        let backup_path = create_backup(&binary_path).unwrap();
        
        // Verify backup exists
        assert!(backup_path.exists());
        assert_eq!(backup_path.extension().unwrap(), "bak");
        
        // Verify content matches
        let backup_content = std::fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "test binary content");
    }
    
    #[test]
    fn test_restore_from_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("test-binary");
        let backup_path = temp_dir.path().join("test-binary.bak");
        
        // Create backup file
        let mut file = std::fs::File::create(&backup_path).unwrap();
        file.write_all(b"backup content").unwrap();
        drop(file);
        
        // Create a different current file
        let mut file = std::fs::File::create(&binary_path).unwrap();
        file.write_all(b"new content").unwrap();
        drop(file);
        
        // Restore from backup
        restore_from_backup(&backup_path, &binary_path).unwrap();
        
        // Verify content was restored
        let content = std::fs::read_to_string(&binary_path).unwrap();
        assert_eq!(content, "backup content");
    }
    
    #[test]
    fn test_verify_installation_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("test-binary");
        
        // Create a file with reasonable size
        let mut file = std::fs::File::create(&binary_path).unwrap();
        file.write_all(&vec![0u8; 150_000]).unwrap();
        drop(file);
        
        // On Unix, set executable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&binary_path, permissions).unwrap();
        }
        
        // Verify
        let result = verify_installation(&binary_path);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_verify_installation_too_small() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("test-binary");
        
        // Create a file that's too small
        let mut file = std::fs::File::create(&binary_path).unwrap();
        file.write_all(b"small").unwrap();
        drop(file);
        
        // Verify should fail
        let result = verify_installation(&binary_path);
        assert!(result.is_err());
    }
}
