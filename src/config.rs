use std::env;
use std::path::Path;

// Default configuration constants
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8080;
pub const DEFAULT_API_BASE_URL: &str = "";
pub const DEFAULT_API_TOKEN: &str = "";
pub const DEFAULT_PUBLIC_BASE_URL: &str = "";
pub const DEFAULT_OWNER_USERNAME: &str = "owner";
pub const DEFAULT_OWNER_PASSWORD: &str = "owner123";
pub const DEFAULT_OWNER_ROLE: &str = "owner";
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
    env::var("API_BASE_URL").unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string())
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
