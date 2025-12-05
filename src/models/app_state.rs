use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::user_record::UserRecord;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<Mutex<HashMap<String, UserRecord>>>,
    pub sessions: Arc<Mutex<HashMap<String, String>>>,
    pub flash_store: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub default_customer_cache: Arc<Mutex<Option<String>>>,
    pub api_base_url: String,
    pub api_token: String,
    pub public_base_url: String,
    pub client: reqwest::Client,
    pub disabled_instances: Arc<std::collections::HashSet<String>>,
}

impl AppState {
    pub fn is_instance_disabled(&self, id: &str) -> bool {
        self.disabled_instances.contains(id)
    }
}
