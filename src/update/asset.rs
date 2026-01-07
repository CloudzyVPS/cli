/// Asset selection and parsing logic
use super::{error::UpdateError, platform::Platform};
use serde::{Deserialize, Serialize};

/// Represents a release asset (binary, checksum file, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// Asset filename
    pub name: String,
    /// Direct download URL
    pub download_url: String,
    /// File size in bytes
    pub size: u64,
    /// MIME content type
    pub content_type: String,
}

/// Parse an asset name to extract version and target
/// 
/// Expected format: `zy-{VERSION}-{TARGET}[.exe]`
/// 
/// # Examples
/// 
/// ```
/// use zy::update::parse_asset_name;
/// 
/// let result = parse_asset_name("zy-1.0.1-x86_64-unknown-linux-gnu");
/// assert_eq!(result, Some(("1.0.1".to_string(), "x86_64-unknown-linux-gnu".to_string())));
/// 
/// let result = parse_asset_name("zy-1.0.1-x86_64-pc-windows-msvc.exe");
/// assert_eq!(result, Some(("1.0.1".to_string(), "x86_64-pc-windows-msvc".to_string())));
/// ```
#[allow(dead_code)]
pub fn parse_asset_name(name: &str) -> Option<(String, String)> {
    // Remove .exe extension if present
    let name = name.strip_suffix(".exe").unwrap_or(name);
    
    // Expected format: zy-{VERSION}-{TARGET}
    // Split by '-' and expect at least 3 parts: "zy", version components, and target components
    let parts: Vec<&str> = name.split('-').collect();
    
    if parts.len() < 3 || parts[0] != "zy" {
        return None;
    }
    
    // Try to find where the version ends and target begins
    // Version is typically X.Y.Z or X.Y.Z-prerelease
    // Target typically starts with architecture (x86_64, aarch64)
    
    // Find the first part that looks like it could be the start of a target triple
    let arch_indicators = ["x86_64", "aarch64", "i686", "armv7"];
    
    let mut version_end_idx = None;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if arch_indicators.iter().any(|&arch| *part == arch) {
            version_end_idx = Some(i);
            break;
        }
    }
    
    if let Some(idx) = version_end_idx {
        let version = parts[1..idx].join("-");
        let target = parts[idx..].join("-");
        
        if !version.is_empty() && !target.is_empty() {
            return Some((version, target));
        }
    }
    
    None
}

/// Select the correct asset for the current platform from a list of assets
/// 
/// # Examples
/// 
/// ```no_run
/// use zy::update::{Asset, Platform, select_asset_for_platform};
/// 
/// let platform = Platform::current();
/// let assets = vec![
///     Asset {
///         name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
///         download_url: "https://example.com/asset".to_string(),
///         size: 1024,
///         content_type: "application/octet-stream".to_string(),
///     },
/// ];
/// 
/// // This will succeed if running on Linux x86_64
/// // let asset = select_asset_for_platform(&assets, &platform).unwrap();
/// ```
#[allow(dead_code)]
pub fn select_asset_for_platform(
    assets: &[Asset],
    platform: &Platform,
) -> Result<Asset, UpdateError> {
    let target_triple = platform.to_target_triple();
    
    tracing::debug!("Selecting asset for platform: {}", target_triple);
    
    for asset in assets {
        // Skip non-binary assets (like SHA256SUMS.txt)
        if !asset.name.starts_with("zy-") {
            continue;
        }
        
        if asset.name == "SHA256SUMS.txt" {
            continue;
        }
        
        // Parse the asset name
        if let Some((_, asset_target)) = parse_asset_name(&asset.name) {
            tracing::debug!("Found asset with target: {}", asset_target);
            
            // Check if target matches
            if asset_target == target_triple {
                // Verify extension matches platform expectations
                if platform.extension.is_some() {
                    if !asset.name.ends_with(".exe") {
                        tracing::debug!("Asset {} missing .exe extension for Windows", asset.name);
                        continue;
                    }
                } else if asset.name.ends_with(".exe") {
                    tracing::debug!("Asset {} has .exe extension for non-Windows", asset.name);
                    continue;
                }
                
                tracing::info!("Selected asset: {}", asset.name);
                return Ok(asset.clone());
            }
        }
    }
    
    Err(UpdateError::NoAssetFound(format!(
        "No matching asset found for platform {}",
        target_triple
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_asset_name_linux() {
        let result = parse_asset_name("zy-1.0.1-x86_64-unknown-linux-gnu");
        assert_eq!(
            result,
            Some(("1.0.1".to_string(), "x86_64-unknown-linux-gnu".to_string()))
        );
    }

    #[test]
    fn test_parse_asset_name_macos() {
        let result = parse_asset_name("zy-1.0.1-aarch64-apple-darwin");
        assert_eq!(
            result,
            Some(("1.0.1".to_string(), "aarch64-apple-darwin".to_string()))
        );
    }

    #[test]
    fn test_parse_asset_name_windows() {
        let result = parse_asset_name("zy-1.0.1-x86_64-pc-windows-msvc.exe");
        assert_eq!(
            result,
            Some(("1.0.1".to_string(), "x86_64-pc-windows-msvc".to_string()))
        );
    }

    #[test]
    fn test_parse_asset_name_with_prerelease() {
        let result = parse_asset_name("zy-1.0.0-beta.1-x86_64-unknown-linux-gnu");
        assert_eq!(
            result,
            Some(("1.0.0-beta.1".to_string(), "x86_64-unknown-linux-gnu".to_string()))
        );
    }

    #[test]
    fn test_parse_asset_name_invalid() {
        assert_eq!(parse_asset_name("invalid-name"), None);
        assert_eq!(parse_asset_name("zy-1.0.1"), None);
        assert_eq!(parse_asset_name("other-1.0.1-x86_64-unknown-linux-gnu"), None);
    }

    #[test]
    fn test_select_asset_for_platform() {
        let assets = vec![
            Asset {
                name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
                download_url: "https://example.com/linux".to_string(),
                size: 1024,
                content_type: "application/octet-stream".to_string(),
            },
            Asset {
                name: "zy-1.0.1-x86_64-pc-windows-msvc.exe".to_string(),
                download_url: "https://example.com/windows".to_string(),
                size: 2048,
                content_type: "application/octet-stream".to_string(),
            },
            Asset {
                name: "SHA256SUMS.txt".to_string(),
                download_url: "https://example.com/checksums".to_string(),
                size: 512,
                content_type: "text/plain".to_string(),
            },
        ];

        // Test Linux
        let linux_platform = Platform {
            target: "x86_64-unknown-linux-gnu".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            extension: None,
        };
        let result = select_asset_for_platform(&assets, &linux_platform).unwrap();
        assert_eq!(result.name, "zy-1.0.1-x86_64-unknown-linux-gnu");

        // Test Windows
        let windows_platform = Platform {
            target: "x86_64-pc-windows-msvc".to_string(),
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
            extension: Some(".exe".to_string()),
        };
        let result = select_asset_for_platform(&assets, &windows_platform).unwrap();
        assert_eq!(result.name, "zy-1.0.1-x86_64-pc-windows-msvc.exe");
    }

    #[test]
    fn test_select_asset_not_found() {
        let assets = vec![Asset {
            name: "zy-1.0.1-x86_64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/linux".to_string(),
            size: 1024,
            content_type: "application/octet-stream".to_string(),
        }];

        let unsupported_platform = Platform {
            target: "arm-unknown-linux-gnueabihf".to_string(),
            os: "linux".to_string(),
            arch: "arm".to_string(),
            extension: None,
        };

        let result = select_asset_for_platform(&assets, &unsupported_platform);
        assert!(result.is_err());
        match result {
            Err(UpdateError::NoAssetFound(_)) => {}
            _ => panic!("Expected NoAssetFound error"),
        }
    }
}
