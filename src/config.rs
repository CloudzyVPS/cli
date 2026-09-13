//! Where zy talks to, and as whom.
//!
//! Resolution order, highest first:
//!   1. command-line flags (`--url`, `--profile`)
//!   2. environment (`CLOUDZY_URL`, `CLOUDZY_PROFILE`, `CLOUDZY_TOKEN`)
//!   3. the profile file, `$CLOUDZY_CONFIG_DIR` or `<config dir>/cloudzy/`
//!   4. built-in defaults (`https://dash.cloudzy.com`, profile `default`)

use std::path::PathBuf;

/// The Cloudzy platform zy talks to when nothing says otherwise.
pub const DEFAULT_URL: &str = "https://dash.cloudzy.com";
/// Profile used when none is named.
pub const DEFAULT_PROFILE: &str = "default";
/// The OAuth client registered for zy on the Cloudzy platform.
pub const CLIENT_ID: &str = "cloudzy-cli";

pub const ENV_URL: &str = "CLOUDZY_URL";
pub const ENV_TOKEN: &str = "CLOUDZY_TOKEN";
pub const ENV_PROFILE: &str = "CLOUDZY_PROFILE";
pub const ENV_CONFIG_DIR: &str = "CLOUDZY_CONFIG_DIR";

/// Resolved settings for one invocation.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Base URL of the Cloudzy platform, without a trailing slash.
    pub url: String,
    /// Named profile the stored sign-in belongs to.
    pub profile: String,
    /// A developer API token from the environment, if one was given.
    pub env_token: Option<String>,
    /// Directory holding `credentials.json`.
    pub config_dir: PathBuf,
}

impl Settings {
    /// Resolve settings from flags, environment and the profile file.
    pub fn resolve(url_flag: Option<&str>, profile_flag: Option<&str>) -> Settings {
        let config_dir = config_dir();
        let profile = profile_flag
            .map(str::to_string)
            .or_else(|| non_empty_env(ENV_PROFILE))
            .unwrap_or_else(|| DEFAULT_PROFILE.to_string());
        let stored_url = crate::auth::store::FileStore::new(config_dir.clone())
            .load(&profile)
            .ok()
            .flatten()
            .map(|p| p.url);
        let url = url_flag
            .map(str::to_string)
            .or_else(|| non_empty_env(ENV_URL))
            .or(stored_url)
            .unwrap_or_else(|| DEFAULT_URL.to_string());
        Settings {
            url: sanitize_base_url(&url),
            profile,
            env_token: non_empty_env(ENV_TOKEN),
            config_dir,
        }
    }
}

/// `$CLOUDZY_CONFIG_DIR`, else the platform config directory + `cloudzy`.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = non_empty_env(ENV_CONFIG_DIR) {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cloudzy")
}

/// Trim whitespace and trailing slashes; an empty value means the default.
pub fn sanitize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
