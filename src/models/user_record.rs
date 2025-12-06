use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub password: String,
    pub role: String,
    pub assigned_instances: Vec<String>,
}
