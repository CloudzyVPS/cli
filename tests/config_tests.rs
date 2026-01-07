use zy::config;
use std::env;

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
    // Set environment variable with trailing slash
    env::set_var("API_BASE_URL", "https://api.cloudzy.com/developers/");
    
    let result = config::get_api_base_url();
    
    assert_eq!(result, "https://api.cloudzy.com/developers");
    
    // Clean up
    env::remove_var("API_BASE_URL");
}

#[test]
fn test_get_api_base_url_without_trailing_slash() {
    // Set environment variable without trailing slash
    env::set_var("API_BASE_URL", "https://api.cloudzy.com/developers");
    
    let result = config::get_api_base_url();
    
    // Should remain unchanged
    assert_eq!(result, "https://api.cloudzy.com/developers");
    
    // Clean up
    env::remove_var("API_BASE_URL");
}

#[test]
fn test_get_api_base_url_uses_default() {
    // Remove environment variable if it exists
    env::remove_var("API_BASE_URL");
    
    let result = config::get_api_base_url();
    
    // DEFAULT_API_BASE_URL is empty, so sanitize_base_url returns localhost fallback
    assert_eq!(result, "http://localhost:5000");
}
