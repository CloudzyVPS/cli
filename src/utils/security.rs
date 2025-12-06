/// Security utilities for environment and configuration validation
use std::path::Path;

/// Validates that sensitive files have appropriate permissions
pub fn validate_file_permissions(file_path: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        
        let path = Path::new(file_path);
        if !path.exists() {
            return Ok(()); // File doesn't exist yet, will be created with proper permissions
        }
        
        match std::fs::metadata(path) {
            Ok(metadata) => {
                let mode = metadata.permissions().mode();
                // Check if file is world-readable (0o004) or world-writable (0o002)
                if mode & 0o006 != 0 {
                    return Err(format!(
                        "Security warning: {} has insecure permissions (mode: {:o}). \
                         Sensitive files should not be readable or writable by others. \
                         Run: chmod 600 {}",
                        file_path, mode, file_path
                    ));
                }
                Ok(())
            }
            Err(e) => Err(format!("Failed to check permissions for {}: {}", file_path, e)),
        }
    }
    
    #[cfg(not(unix))]
    {
        // On non-Unix systems (Windows), we can't easily check permissions
        // Log a warning instead
        tracing::info!("File permission check skipped on non-Unix system for {}", file_path);
        Ok(())
    }
}

/// Validates that API token is not empty and meets minimum security requirements
pub fn validate_api_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("API_TOKEN is not configured".to_string());
    }
    
    if token.len() < 32 {
        return Err("API_TOKEN appears to be too short (minimum 32 characters recommended)".to_string());
    }
    
    // Check if token looks like a placeholder
    let placeholders = ["your_token_here", "your_api_token_here", "replace_me", "changeme"];
    let lower_token = token.to_lowercase();
    if placeholders.iter().any(|p| lower_token.contains(p)) {
        return Err("API_TOKEN appears to be a placeholder value. Please set a real token.".to_string());
    }
    
    Ok(())
}

/// Checks if running in development mode (localhost bindings)
pub fn is_development_mode(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_api_token() {
        assert!(validate_api_token("").is_err());
        assert!(validate_api_token("short").is_err());
        assert!(validate_api_token("your_token_here_padding_to_make_it_long_enough").is_err());
        assert!(validate_api_token("a".repeat(32).as_str()).is_ok());
    }
    
    #[test]
    fn test_is_development_mode() {
        assert!(is_development_mode("127.0.0.1"));
        assert!(is_development_mode("localhost"));
        assert!(is_development_mode("::1"));
        assert!(!is_development_mode("0.0.0.0"));
        assert!(!is_development_mode("192.168.1.1"));
    }
}
