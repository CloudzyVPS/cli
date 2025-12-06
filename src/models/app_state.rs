use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::user_record::UserRecord;
use crate::models::session::Session;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<Mutex<HashMap<String, UserRecord>>>,
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub flash_store: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub default_customer_cache: Arc<Mutex<Option<String>>>,
    pub api_base_url: String,
    pub api_token: String,
    pub public_base_url: String,
    pub client: reqwest::Client,
    pub disabled_instances: Arc<std::collections::HashSet<String>>,
    pub custom_css: Option<String>,
}

impl AppState {
    pub fn is_instance_disabled(&self, id: &str) -> bool {
        self.disabled_instances.contains(id)
    }
    
    /// Clean up expired sessions
    #[allow(dead_code)]
    pub fn cleanup_expired_sessions(&self, max_age_seconds: u64, idle_timeout_seconds: u64) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.retain(|_, session| {
                !session.is_expired(max_age_seconds) && !session.is_idle(idle_timeout_seconds)
            });
        } else {
            tracing::error!("Failed to acquire sessions lock during cleanup");
        }
    }
}
