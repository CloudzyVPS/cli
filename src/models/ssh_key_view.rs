use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyView {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub customer_id: Option<String>,
}
