use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyDisplay {
    pub id: String,
    pub name: String,
    pub selected: bool,
}
