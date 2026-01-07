use std::env;
use std::path::Path;

// Default configuration constants
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 5000;
pub const DEFAULT_API_BASE_URL: &str = "";
pub const DEFAULT_API_TOKEN: &str = "";
pub const DEFAULT_PUBLIC_BASE_URL: &str = "";
pub const DEFAULT_OWNER_USERNAME: &str = "owner";
pub const DEFAULT_OWNER_PASSWORD: &str = "owner123";
pub const DEFAULT_OWNER_ROLE: &str = "owner";
#[allow(dead_code)]
pub const DEFAULT_ADMIN_ROLE: &str = "admin";
pub const DEFAULT_PBKDF2_ITERATIONS: u32 = 100_000;

pub fn load_env_file(env_file: Option<&str>) {
    if let Some(path) = env_file {
        dotenvy::from_path(Path::new(path)).ok();
    } else {
        dotenvy::dotenv().ok();
    }
}

pub fn get_api_base_url() -> String {
    sanitize_base_url(&env::var("API_BASE_URL").unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string()))
}

pub fn get_api_token() -> String {
    env::var("API_TOKEN").unwrap_or_else(|_| DEFAULT_API_TOKEN.to_string())
}

pub fn get_public_base_url() -> String {
    sanitize_base_url(&env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| DEFAULT_PUBLIC_BASE_URL.to_string()))
}

pub fn get_disabled_instance_ids() -> std::collections::HashSet<String> {
    let raw = env::var("DISABLED_INSTANCE_IDS").unwrap_or_default();
    let mut set = std::collections::HashSet::new();
    if !raw.trim().is_empty() {
        for id in raw.split(',') {
            let t = id.trim();
            if !t.is_empty() {
                set.insert(t.to_string());
            }
        }
    }
    set
}

fn sanitize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "http://localhost:5000".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_base_url_removes_trailing_slash() {
        assert_eq!(
            sanitize_base_url("https://api.cloudzy.com/developers/"),
            "https://api.cloudzy.com/developers"
        );
    }

    #[test]
    fn test_sanitize_base_url_no_trailing_slash() {
        assert_eq!(
            sanitize_base_url("https://api.cloudzy.com/developers"),
            "https://api.cloudzy.com/developers"
        );
    }

    #[test]
    fn test_sanitize_base_url_multiple_trailing_slashes() {
        assert_eq!(
            sanitize_base_url("https://api.cloudzy.com/developers///"),
            "https://api.cloudzy.com/developers"
        );
    }

    #[test]
    fn test_sanitize_base_url_with_whitespace() {
        assert_eq!(
            sanitize_base_url("  https://api.cloudzy.com/developers/  "),
            "https://api.cloudzy.com/developers"
        );
    }

    #[test]
    fn test_sanitize_base_url_empty_string() {
        assert_eq!(
            sanitize_base_url(""),
            "http://localhost:5000"
        );
    }

    #[test]
    fn test_sanitize_base_url_whitespace_only() {
        assert_eq!(
            sanitize_base_url("   "),
            "http://localhost:5000"
        );
    }

    #[test]
    fn test_get_api_base_url_with_trailing_slash() {
        // Set environment variable with trailing slash
        env::set_var("API_BASE_URL", "https://api.cloudzy.com/developers/");
        
        let result = get_api_base_url();
        
        // Should remove trailing slash after the fix
        // Before fix: https://api.cloudzy.com/developers/
        // After fix: https://api.cloudzy.com/developers
        assert_eq!(result, "https://api.cloudzy.com/developers");
        
        // Clean up
        env::remove_var("API_BASE_URL");
    }

    #[test]
    fn test_get_api_base_url_without_trailing_slash() {
        // Set environment variable without trailing slash
        env::set_var("API_BASE_URL", "https://api.cloudzy.com/developers");
        
        let result = get_api_base_url();
        
        // Should remain unchanged
        assert_eq!(result, "https://api.cloudzy.com/developers");
        
        // Clean up
        env::remove_var("API_BASE_URL");
    }

    #[test]
    fn test_get_api_base_url_uses_default() {
        // Remove environment variable if it exists
        env::remove_var("API_BASE_URL");
        
        let result = get_api_base_url();
        
        // DEFAULT_API_BASE_URL is empty, so sanitize_base_url returns localhost fallback
        assert_eq!(result, "http://localhost:5000");
    }
}
