use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::user_record::UserRecord;
use crate::models::workspace_record::WorkspaceRecord;
use crate::mcp::log::McpLogStore;

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
    pub disabled_instances: Arc<Mutex<std::collections::HashSet<String>>>,
    pub current_hostname: String,
    pub custom_css: Option<String>,
    /// All workspaces keyed by slug.
    pub workspaces: Arc<Mutex<HashMap<String, WorkspaceRecord>>>,
    /// Shared MCP call log store (populated by the stdio MCP server, read by the web UI).
    pub mcp_log_store: McpLogStore,
}

impl AppState {
    pub fn is_instance_disabled(&self, id: &str) -> bool {
        self.disabled_instances.lock().unwrap().contains(id)
    }

    pub fn is_hostname_blocked(&self, instance_hostname: &str) -> bool {
        if self.current_hostname.is_empty() || instance_hostname.is_empty() {
            return false;
        }
        self.current_hostname.to_lowercase() == instance_hostname.to_lowercase()
    }
}
