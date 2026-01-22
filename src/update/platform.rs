/// Platform detection for cross-platform updates
use super::error::UpdateError;

/// Represents the current platform's details
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
