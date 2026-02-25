use zy::config;
use std::env;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value);
        EnvGuard { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(val) = &self.previous {
            env::set_var(self.key, val);
        } else {
            env::remove_var(self.key);
        }
    }
}

#[test]
fn test_sanitize_base_url_removes_trailing_slash() {
    assert_eq!(
        config::sanitize_base_url("https://api.cloudzy.com/developers/"),
        "https://api.cloudzy.com/developers"
    );
}

#[test]
fn test_sanitize_base_url_no_trailing_slash() {
    assert_eq!(
        config::sanitize_base_url("https://api.cloudzy.com/developers"),
        "https://api.cloudzy.com/developers"
    );
}

#[test]
fn test_sanitize_base_url_multiple_trailing_slashes() {
    assert_eq!(
        config::sanitize_base_url("https://api.cloudzy.com/developers///"),
        "https://api.cloudzy.com/developers"
    );
}

#[test]
fn test_sanitize_base_url_with_whitespace() {
    assert_eq!(
        config::sanitize_base_url("  https://api.cloudzy.com/developers/  "),
        "https://api.cloudzy.com/developers"
    );
}

#[test]
fn test_sanitize_base_url_empty_string() {
    assert_eq!(
        config::sanitize_base_url(""),
        "http://localhost:5000"
    );
}

#[test]
fn test_sanitize_base_url_whitespace_only() {
    assert_eq!(
        config::sanitize_base_url("   "),
        "http://localhost:5000"
    );
}

#[test]
fn test_get_api_base_url_with_trailing_slash() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let _guard = EnvGuard::set("API_BASE_URL", "https://api.cloudzy.com/developers/");

    let result = config::get_api_base_url();

    assert_eq!(result, "https://api.cloudzy.com/developers");
}

#[test]
fn test_get_api_base_url_without_trailing_slash() {
    // Set environment variable without trailing slash
    let _lock = ENV_MUTEX.lock().unwrap();
    let _guard = EnvGuard::set("API_BASE_URL", "https://api.cloudzy.com/developers");

    let result = config::get_api_base_url();

    assert_eq!(result, "https://api.cloudzy.com/developers");
}

#[test]
fn test_get_api_base_url_uses_default() {
    // Remove environment variable if it exists
    let _lock = ENV_MUTEX.lock().unwrap();
    env::remove_var("API_BASE_URL");

    let result = config::get_api_base_url();

    assert_eq!(result, "http://localhost:5000");
}

#[test]
fn test_get_disabled_instance_ids_empty() {
    let _lock = ENV_MUTEX.lock().unwrap();
    env::remove_var("DISABLED_INSTANCE_IDS");

    let result = config::get_disabled_instance_ids();

    assert!(result.is_empty());
}

#[test]
fn test_get_disabled_instance_ids_single() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let _guard = EnvGuard::set("DISABLED_INSTANCE_IDS", "abc-123");

    let result = config::get_disabled_instance_ids();

    assert_eq!(result.len(), 1);
    assert!(result.contains("abc-123"));
}

#[test]
fn test_get_disabled_instance_ids_multiple() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let _guard = EnvGuard::set("DISABLED_INSTANCE_IDS", "id-1,id-2, id-3 ");

    let result = config::get_disabled_instance_ids();

    assert_eq!(result.len(), 3);
    assert!(result.contains("id-1"));
    assert!(result.contains("id-2"));
    assert!(result.contains("id-3"));
}
