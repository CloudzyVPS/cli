use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in seconds, with fallback to 0 if system clock fails
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|e| {
            tracing::error!("System clock error: {}. Using fallback timestamp.", e);
            0
        })
}

/// Session data with expiration tracking
#[derive(Clone, Debug)]
pub struct Session {
    pub username: String,
    pub created_at: u64,
    pub last_accessed: u64,
}

impl Session {
    pub fn new(username: String) -> Self {
        let now = current_timestamp();
        
        Self {
            username,
            created_at: now,
            last_accessed: now,
        }
    }
    
    pub fn update_last_accessed(&mut self) {
        self.last_accessed = current_timestamp();
    }
    
    /// Check if session has expired (24 hours by default)
    pub fn is_expired(&self, max_age_seconds: u64) -> bool {
        let now = current_timestamp();
        now.saturating_sub(self.created_at) > max_age_seconds
    }
    
    /// Check if session has been idle too long (2 hours by default)
    pub fn is_idle(&self, idle_timeout_seconds: u64) -> bool {
        let now = current_timestamp();
        now.saturating_sub(self.last_accessed) > idle_timeout_seconds
    }
}
