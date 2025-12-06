use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrentUser {
    pub username: String,
    pub role: String,
}
