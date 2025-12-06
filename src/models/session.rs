use std::time::{SystemTime, UNIX_EPOCH};

/// Session data with expiration tracking
#[derive(Clone, Debug)]
pub struct Session {
    pub username: String,
    pub created_at: u64,
    pub last_accessed: u64,
}

impl Session {
    pub fn new(username: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        
        Self {
            username,
            created_at: now,
            last_accessed: now,
        }
    }
    
    pub fn update_last_accessed(&mut self) {
        self.last_accessed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
    }
    
    /// Check if session has expired (24 hours by default)
    pub fn is_expired(&self, max_age_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        
        now - self.created_at > max_age_seconds
    }
    
    /// Check if session has been idle too long (2 hours by default)
    pub fn is_idle(&self, idle_timeout_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        
        now - self.last_accessed > idle_timeout_seconds
    }
}
