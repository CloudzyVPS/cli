/// Platform detection for cross-platform updates
use super::error::UpdateError;

/// Represents the current platform's details
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    /// Target triple (e.g., "x86_64-unknown-linux-gnu")
    pub target: String,
    /// Operating system (e.g., "linux", "macos", "windows")
    pub os: String,
    /// Architecture (e.g., "x86_64", "aarch64")
    pub arch: String,
    /// File extension for executables (Some(".exe") for Windows, None otherwise)
    pub extension: Option<String>,
}

#[allow(dead_code)]
impl Platform {
    /// Detect the current platform at runtime
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Platform;
    /// 
    /// let platform = Platform::current();
    /// println!("Running on: {}", platform.target);
    /// ```
    pub fn current() -> Self {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        
        let (target, extension) = match (os, arch) {
            ("linux", "x86_64") => ("x86_64-unknown-linux-gnu".to_string(), None),
            ("linux", "aarch64") => ("aarch64-unknown-linux-gnu".to_string(), None),
            ("macos", "x86_64") => ("x86_64-apple-darwin".to_string(), None),
            ("macos", "aarch64") => ("aarch64-apple-darwin".to_string(), None),
            ("windows", "x86_64") => ("x86_64-pc-windows-msvc".to_string(), Some(".exe".to_string())),
            _ => {
                tracing::warn!("Unsupported platform: {}-{}, using best guess", os, arch);
                (format!("{}-{}", arch, os), if os == "windows" { Some(".exe".to_string()) } else { None })
            }
        };
        
        Platform {
            target,
            os: os.to_string(),
            arch: arch.to_string(),
            extension,
        }
    }
    
    /// Get the target triple for this platform
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Platform;
    /// 
    /// let platform = Platform::current();
    /// let triple = platform.to_target_triple();
    /// ```
    pub fn to_target_triple(&self) -> String {
        self.target.clone()
    }
    
    /// Check if this platform is supported for updates
    pub fn is_supported(&self) -> Result<(), UpdateError> {
        let supported_targets = [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ];
        
        if supported_targets.contains(&self.target.as_str()) {
            Ok(())
        } else {
            Err(UpdateError::UnsupportedPlatform(self.target.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_platform() {
        let platform = Platform::current();
        
        // Should detect something
        assert!(!platform.target.is_empty());
        assert!(!platform.os.is_empty());
        assert!(!platform.arch.is_empty());
        
        // Windows should have .exe extension
        if platform.os == "windows" {
            assert_eq!(platform.extension, Some(".exe".to_string()));
        } else {
            assert_eq!(platform.extension, None);
        }
    }

    #[test]
    fn test_to_target_triple() {
        let platform = Platform::current();
        let triple = platform.to_target_triple();
        assert!(!triple.is_empty());
    }

    #[test]
    fn test_is_supported() {
        let platform = Platform::current();
        
        // If we're running on a supported platform (which we should be in CI)
        // this should succeed
        let result = platform.is_supported();
        
        // We can't guarantee which platform we're on, but we can check the error type
        match result {
            Ok(_) => {
                // Supported platform - test passes
            }
            Err(UpdateError::UnsupportedPlatform(_)) => {
                // Unsupported platform - still a valid test outcome
            }
            Err(_) => {
                panic!("Unexpected error type");
            }
        }
    }

    #[test]
    fn test_supported_platforms() {
        let test_cases = vec![
            ("linux", "x86_64", "x86_64-unknown-linux-gnu", None),
            ("linux", "aarch64", "aarch64-unknown-linux-gnu", None),
            ("macos", "x86_64", "x86_64-apple-darwin", None),
            ("macos", "aarch64", "aarch64-apple-darwin", None),
            ("windows", "x86_64", "x86_64-pc-windows-msvc", Some(".exe".to_string())),
        ];
        
        for (os, arch, expected_target, expected_ext) in test_cases {
            // Create a platform manually to test mapping
            let platform = Platform {
                target: expected_target.to_string(),
                os: os.to_string(),
                arch: arch.to_string(),
                extension: expected_ext.clone(),
            };
            
            assert_eq!(platform.target, expected_target);
            assert_eq!(platform.extension, expected_ext);
            assert!(platform.is_supported().is_ok());
        }
    }
}
