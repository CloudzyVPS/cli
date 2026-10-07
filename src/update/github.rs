/// GitHub Releases API client
use super::{asset::Asset, channel::Channel, error::UpdateError, version::Version};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    pub prerelease: bool,
    /// Release assets (binaries, checksums, etc.)
    pub assets: Vec<Asset>,
    /// Direct download URL for the release page
    pub download_url: String,
}

/// GitHub Releases API client
pub struct GitHubClient {
    pub repo_owner: String,
    pub repo_name: String,
    pub client: reqwest::Client,
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
        headers.insert(USER_AGENT, HeaderValue::from_static("zy-cli-updater/1.0"));

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

        self.get_releases_at(&url).await
    }

    async fn get_releases_at(&self, url: &str) -> Result<Vec<Release>, UpdateError> {
        tracing::debug!("GET {url}");

        let response = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| UpdateError::Network(e.to_string()))?;

        // Check rate limiting
        self.check_rate_limit(&response)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            tracing::debug!("GitHub HTTP {status}: {error_text}");

            return Err(UpdateError::GitHubApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| UpdateError::Network(e.to_string()))?;

        tracing::debug!("GitHub releases response: {text}");

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
        tracing::debug!(
            "Found {} releases matching channel {:?}",
            filtered.len(),
            channel
        );

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
            tracing::debug!(
                "Latest release for channel {:?}: {} (tag: {})",
                channel,
                release.version,
                release.tag_name
            );
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
                            .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn official_v2_release_is_parsed_and_its_binary_is_discoverable() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../../tests/fixtures/release-v2.0.json"),
                "application/json",
            ))
            .expect(1)
            .mount(&mock)
            .await;
        let client = GitHubClient::new("CloudzyVPS".into(), "cli".into());
        let releases = client
            .get_releases_at(&format!("{}/releases", mock.uri()))
            .await
            .unwrap();
        assert_eq!(releases.len(), 1);
        let release = &releases[0];
        assert_eq!(release.version.to_string(), "2.0.0");
        assert!(release
            .version
            .is_newer_than(&Version::parse("1.0.2").unwrap()));
        assert_eq!(Channel::from_version(&release.tag_name), Channel::Stable);
        assert!(super::super::select_asset_for_platform(
            &release.assets,
            &super::super::Platform::current()
        )
        .is_ok());
    }
}
