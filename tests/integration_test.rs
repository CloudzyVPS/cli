/// Integration test for update functionality
use zy::update::{Channel, Platform, Version, check_for_update, select_asset_for_platform};

#[test]
fn test_version_comparison() {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.0.1").unwrap();
    
    assert!(v2.is_newer_than(&v1));
}

#[test]
fn test_platform_detection_works() {
    let platform = Platform::current();
    println!("Detected platform: {:?}", platform);
    
    // Should be able to detect a platform
    assert!(!platform.target.is_empty());
}

#[test]
fn test_channel_detection() {
    assert_eq!(Channel::from_version("1.0.0"), Channel::Stable);
    assert_eq!(Channel::from_version("1.0.0-beta"), Channel::Beta);
    assert_eq!(Channel::from_version("1.0.0-alpha"), Channel::Alpha);
    assert_eq!(Channel::from_version("1.0.0-rc.1"), Channel::ReleaseCandidate);
}

// This test is ignored by default to avoid rate limiting
// Run with: cargo test -- --ignored
#[tokio::test]
#[ignore]
async fn test_github_api_integration() {
    use zy::update::GitHubClient;
    
    let client = GitHubClient::new("CloudzyVPS".to_string(), "cli".to_string());
    
    // Try to fetch releases
    match client.get_all_releases().await {
        Ok(releases) => {
            println!("Found {} releases", releases.len());
            if !releases.is_empty() {
                println!("Latest release: {}", releases[0].version);
            }
        }
        Err(e) => {
            println!("Failed to fetch releases: {}", e);
            // Don't fail the test if we hit rate limiting or network issues
        }
    }
}

// This test is ignored by default to avoid rate limiting
// Run with: cargo test -- --ignored
#[tokio::test]
#[ignore]
async fn test_check_for_update_integration() {
    match check_for_update(Channel::Stable).await {
        Ok(Some(release)) => {
            println!("Update available: {}", release.version);
            println!("Download URL: {}", release.download_url);
            
            // Try to select an asset for our platform
            let platform = Platform::current();
            match select_asset_for_platform(&release.assets, &platform) {
                Ok(asset) => {
                    println!("Asset found: {}", asset.name);
                    println!("Download URL: {}", asset.download_url);
                }
                Err(e) => {
                    println!("No asset for platform {}: {}", platform.target, e);
                }
            }
        }
        Ok(None) => {
            println!("Already on latest version");
        }
        Err(e) => {
            println!("Error checking for updates: {}", e);
        }
    }
}
