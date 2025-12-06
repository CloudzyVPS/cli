use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub username: String,
    pub role: String,
    pub assigned: String,
}
