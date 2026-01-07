/// GitHub Releases API client
use super::{asset::Asset, channel::Channel, error::UpdateError, version::Version};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use yansi::Paint;

/// GitHub API release response
#[derive(Debug, Clone, Deserialize, Serialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

/// GitHub API asset response
#[derive(Debug, Clone, Deserialize, Serialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    content_type: String,
}

/// Represents a GitHub release with parsed version information
#[derive(Debug, Clone)]
pub struct Release {
    /// Git tag name (e.g., "v1.0.1")
    pub tag_name: String,
    /// Parsed semantic version
    pub version: Version,
    /// Whether this is a pre-release
    #[allow(dead_code)]
    pub prerelease: bool,
    /// Release assets (binaries, checksums, etc.)
    #[allow(dead_code)]
    pub assets: Vec<Asset>,
    /// Direct download URL for the release page
    pub download_url: String,
}

/// GitHub Releases API client
pub struct GitHubClient {
    repo_owner: String,
    repo_name: String,
    client: reqwest::Client,
}

impl GitHubClient {
    /// Create a new GitHub API client
    /// 
    /// # Examples
    /// 
    /// ```
    /// use zy::update::GitHubClient;
    /// 
    /// let client = GitHubClient::new("CloudzyVPS".to_string(), "cli".to_string());
    /// ```
    pub fn new(repo_owner: String, repo_name: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("zy-cli-updater/1.0"),
        );
        
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            repo_owner,
            repo_name,
            client,
        }
    }
    
    /// Get all releases from the repository
    /// 
    /// # Errors
    /// 
    /// Returns `UpdateError::Network` for network failures,
    /// `UpdateError::RateLimitExceeded` for rate limiting,
    /// or `UpdateError::GitHubApiError` for API errors.
    pub async fn get_all_releases(&self) -> Result<Vec<Release>, UpdateError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases",
            self.repo_owner, self.repo_name
        );
        
        // --- Curl Logging ---
        let mut parts = Vec::new();
        parts.push(Paint::new("curl").fg(yansi::Color::Green).bold().to_string());
        parts.push(format!("-X {}", Paint::new("GET").fg(yansi::Color::Yellow).bold()));
        parts.push(format!("'{}'", Paint::new(&url).fg(yansi::Color::Cyan)));
        parts.push(format!("{} {}", 
            Paint::new("-H").fg(yansi::Color::Magenta), 
            Paint::new("'Accept: application/vnd.github.v3+json'").fg(yansi::Color::Magenta)
        ));
        
        println!("Request:\n{}", parts.join(" "));
        // --------------------
        
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        
        // Check rate limiting
        if let Err(e) = self.check_rate_limit(&response) {
            return Err(e);
        }
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            
            println!("Response:\n{}", Paint::new(format!("HTTP {}: {}", status, error_text)).fg(yansi::Color::Red));
            
            return Err(UpdateError::GitHubApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }
        
        let text = response.text().await.map_err(|e| UpdateError::Network(e.to_string()))?;
        
        // Colorize the response JSON for better readability in the terminal
        // Grayed out color (dimmed/dark gray)
        let response_str = Paint::new(&text).rgb(100, 100, 100).to_string();
        println!("Response:\n{}", response_str);
        
        let github_releases: Vec<GitHubRelease> = serde_json::from_str(&text)
            .map_err(|e| UpdateError::GitHubApiError(format!("Failed to parse JSON: {}", e)))?;
        
        tracing::debug!("Found {} releases", github_releases.len());
        
        let mut releases = Vec::new();
        for gh_release in github_releases {
            // Try to parse the version from the tag
            match Version::parse(&gh_release.tag_name) {
                Ok(version) => {
                    let assets = gh_release
                        .assets
                        .into_iter()
                        .map(|a| Asset {
                            name: a.name,
                            download_url: a.browser_download_url,
                            size: a.size,
                            content_type: a.content_type,
                        })
                        .collect();
                    
                    releases.push(Release {
                        tag_name: gh_release.tag_name.clone(),
                        version,
                        prerelease: gh_release.prerelease,
                        assets,
                        download_url: format!(
                            "https://github.com/{}/{}/releases/tag/{}",
                            self.repo_owner, self.repo_name, gh_release.tag_name
                        ),
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "Skipping release {} - invalid version: {}",
                        gh_release.tag_name,
                        e
                    );
                }
            }
        }
        
        Ok(releases)
    }
    
    /// Get the latest release for a specific channel
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use zy::update::{GitHubClient, Channel};
    /// 
    /// # async fn example() {
    /// let client = GitHubClient::new("CloudzyVPS".to_string(), "cli".to_string());
    /// let release = client.get_latest_release(Channel::Stable).await.unwrap();
    /// println!("Latest stable: {}", release.version);
    /// # }
    /// ```
    pub async fn get_latest_release(&self, channel: Channel) -> Result<Release, UpdateError> {
        let releases = self.get_all_releases().await?;
        
        if releases.is_empty() {
            return Err(UpdateError::GitHubApiError(format!(
                "No releases found in the repository {}/{}",
                self.repo_owner, self.repo_name
            )));
        }
        
        // Filter releases by channel
        let filtered: Vec<_> = releases
            .into_iter()
            .filter(|r| {
                let release_channel = Channel::from_version(&r.tag_name);
                
                match channel {
                    Channel::Stable => release_channel == Channel::Stable,
                    _ => {
                        // For pre-release channels, include the specific channel
                        release_channel == channel
                    }
                }
            })
            .collect();
        
        tracing::debug!(
            "Found {} releases for channel {:?}",
            filtered.len(),
            channel
        );
        println!("Found {} releases matching channel {:?}", filtered.len(), channel);
        
        // Find the newest version
        let latest = filtered
            .into_iter()
            .max_by(|a, b| {
                if a.version.is_newer_than(&b.version) {
                    std::cmp::Ordering::Greater
                } else if b.version.is_newer_than(&a.version) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok_or(UpdateError::NoReleaseFound(channel));

        if let Ok(ref release) = latest {
            println!("Latest release for channel {:?}: {} (tag: {})", channel, release.version, release.tag_name);
        }

        latest
    }
    
    /// Check rate limiting headers and return error if exceeded
    fn check_rate_limit(&self, response: &reqwest::Response) -> Result<(), UpdateError> {
        if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
            if let Ok(remaining_str) = remaining.to_str() {
                if let Ok(remaining_count) = remaining_str.parse::<u32>() {
                    tracing::debug!("GitHub API rate limit remaining: {}", remaining_count);
                    
                    if remaining_count == 0 {
                        let reset_time = response
                            .headers()
                            .get("x-ratelimit-reset")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<i64>().ok())
                            .and_then(|timestamp| {
                                chrono::DateTime::from_timestamp(timestamp, 0)
                            })
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_else(|| "unknown".to_string());
                        
                        return Err(UpdateError::RateLimitExceeded { reset_time });
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GitHubClient::new("CloudzyVPS".to_string(), "cli".to_string());
        assert_eq!(client.repo_owner, "CloudzyVPS");
        assert_eq!(client.repo_name, "cli");
    }

    // Note: Integration tests that actually call the GitHub API should be
    // run sparingly to avoid rate limiting. Consider mocking for unit tests.
}
