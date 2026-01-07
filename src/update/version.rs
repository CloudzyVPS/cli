/// Semantic version parsing and comparison
use super::error::UpdateError;

/// Represents a semantic version with optional pre-release tag
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Major version number
    pub major: u64,
    /// Minor version number
    pub minor: u64,
    /// Patch version number
    pub patch: u64,
    /// Optional pre-release tag (e.g., "alpha.1", "beta", "rc.1")
    pub pre_release: Option<String>,
}

impl Version {
    /// Parse a semantic version string
    /// 
    /// Accepts versions with or without 'v' prefix, and with optional pre-release tags.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Version;
    /// 
    /// let v1 = Version::parse("1.0.0").unwrap();
    /// assert_eq!(v1.major, 1);
    /// assert_eq!(v1.minor, 0);
    /// assert_eq!(v1.patch, 0);
    /// 
    /// let v2 = Version::parse("v2.1.3").unwrap();
    /// assert_eq!(v2.major, 2);
    /// 
    /// let v3 = Version::parse("1.0.0-beta.1").unwrap();
    /// assert_eq!(v3.pre_release, Some("beta.1".to_string()));
    /// ```
    pub fn parse(s: &str) -> Result<Self, UpdateError> {
        // Remove 'v' prefix if present
        let s = s.strip_prefix('v').unwrap_or(s);
        
        // Split on '-' to separate version from pre-release
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        let version_part = parts[0];
        let pre_release = parts.get(1).map(|s| s.to_string());
        
        // Parse version numbers
        let version_nums: Vec<&str> = version_part.split('.').collect();
        if version_nums.len() != 3 {
            return Err(UpdateError::InvalidVersion(
                format!("Expected 3 version components, got {}", version_nums.len())
            ));
        }
        
        let major = version_nums[0]
            .parse::<u64>()
            .map_err(|_| UpdateError::InvalidVersion(format!("Invalid major version: {}", version_nums[0])))?;
        
        let minor = version_nums[1]
            .parse::<u64>()
            .map_err(|_| UpdateError::InvalidVersion(format!("Invalid minor version: {}", version_nums[1])))?;
        
        let patch = version_nums[2]
            .parse::<u64>()
            .map_err(|_| UpdateError::InvalidVersion(format!("Invalid patch version: {}", version_nums[2])))?;
        
        Ok(Version {
            major,
            minor,
            patch,
            pre_release,
        })
    }
    
    /// Get the current binary version from Cargo package metadata
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Version;
    /// 
    /// let current = Version::current();
    /// // Current version is determined at compile time from Cargo.toml
    /// ```
    pub fn current() -> Self {
        // This will never fail since CARGO_PKG_VERSION is always valid
        Self::parse(env!("CARGO_PKG_VERSION"))
            .expect("CARGO_PKG_VERSION should always be valid semver")
    }
    
    /// Check if this version is newer than another version
    /// 
    /// Pre-release versions are considered older than stable versions with the same numbers.
    /// For example: 1.0.0-beta < 1.0.0
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Version;
    /// 
    /// let v1 = Version::parse("1.0.0").unwrap();
    /// let v2 = Version::parse("1.0.1").unwrap();
    /// let v3 = Version::parse("2.0.0").unwrap();
    /// 
    /// assert!(v2.is_newer_than(&v1));
    /// assert!(v3.is_newer_than(&v2));
    /// assert!(!v1.is_newer_than(&v2));
    /// ```
    pub fn is_newer_than(&self, other: &Version) -> bool {
        // Compare major version
        if self.major != other.major {
            return self.major > other.major;
        }
        
        // Compare minor version
        if self.minor != other.minor {
            return self.minor > other.minor;
        }
        
        // Compare patch version
        if self.patch != other.patch {
            return self.patch > other.patch;
        }
        
        // If versions are equal, check pre-release tags
        // A version without pre-release is considered newer than one with pre-release
        match (&self.pre_release, &other.pre_release) {
            (None, Some(_)) => true,  // Stable is newer than pre-release
            (Some(_), None) => false, // Pre-release is older than stable
            _ => false,               // Equal or both have pre-release (consider equal)
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre_release {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_version() {
        let v = Version::parse("1.0.0").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.pre_release, None);
    }

    #[test]
    fn test_parse_with_v_prefix() {
        let v = Version::parse("v2.1.3").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_parse_with_prerelease() {
        let v = Version::parse("1.0.0-beta.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.pre_release, Some("beta.1".to_string()));
    }

    #[test]
    fn test_parse_with_v_prefix_and_prerelease() {
        let v = Version::parse("v1.2.3-rc.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre_release, Some("rc.1".to_string()));
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(Version::parse("1.0").is_err());
        assert!(Version::parse("1.0.0.0").is_err());
        assert!(Version::parse("a.b.c").is_err());
    }

    #[test]
    fn test_current_version() {
        let current = Version::current();
        // Should not panic and should be valid
        // Note: major, minor, patch are u64 so they're always >= 0
        assert!(current.to_string().contains('.'));
    }

    #[test]
    fn test_is_newer_than_major() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        assert!(v2.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_is_newer_than_minor() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.1.0").unwrap();
        assert!(v2.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_is_newer_than_patch() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.1").unwrap();
        assert!(v2.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
    }

    #[test]
    fn test_is_newer_than_prerelease() {
        let stable = Version::parse("1.0.0").unwrap();
        let beta = Version::parse("1.0.0-beta").unwrap();
        
        // Stable is newer than pre-release
        assert!(stable.is_newer_than(&beta));
        assert!(!beta.is_newer_than(&stable));
    }

    #[test]
    fn test_is_newer_than_equal() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.0").unwrap();
        assert!(!v1.is_newer_than(&v2));
        assert!(!v2.is_newer_than(&v1));
    }

    #[test]
    fn test_display() {
        let v1 = Version::parse("1.0.0").unwrap();
        assert_eq!(v1.to_string(), "1.0.0");
        
        let v2 = Version::parse("1.0.0-beta.1").unwrap();
        assert_eq!(v2.to_string(), "1.0.0-beta.1");
    }
}
