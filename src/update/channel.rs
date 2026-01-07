/// Release channel management
use serde::{Deserialize, Serialize};

/// Release channels for updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    /// Stable releases (no pre-release tags)
    Stable,
    /// Beta releases (tags containing "beta")
    Beta,
    /// Alpha releases (tags containing "alpha")
    Alpha,
    /// Release candidates (tags containing "rc")
    ReleaseCandidate,
}

impl Channel {
    /// Detect channel from a version string or tag name
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Channel;
    /// 
    /// assert_eq!(Channel::from_version("1.0.0"), Channel::Stable);
    /// assert_eq!(Channel::from_version("1.0.0-alpha.1"), Channel::Alpha);
    /// assert_eq!(Channel::from_version("v1.0.0-beta"), Channel::Beta);
    /// assert_eq!(Channel::from_version("1.0.0-rc.1"), Channel::ReleaseCandidate);
    /// ```
    pub fn from_version(version: &str) -> Self {
        let lower = version.to_lowercase();
        
        if lower.contains("alpha") {
            Channel::Alpha
        } else if lower.contains("beta") {
            Channel::Beta
        } else if lower.contains("rc") {
            Channel::ReleaseCandidate
        } else {
            Channel::Stable
        }
    }
    
    /// Check if this channel should include pre-release versions
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::Channel;
    /// 
    /// assert_eq!(Channel::Stable.should_include_prerelease(), false);
    /// assert_eq!(Channel::Alpha.should_include_prerelease(), true);
    /// assert_eq!(Channel::Beta.should_include_prerelease(), true);
    /// assert_eq!(Channel::ReleaseCandidate.should_include_prerelease(), true);
    /// ```
    #[allow(dead_code)]
    pub fn should_include_prerelease(&self) -> bool {
        !matches!(self, Channel::Stable)
    }
}
