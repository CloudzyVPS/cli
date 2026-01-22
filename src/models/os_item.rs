use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsItem {
    pub id: String,
    pub name: String,
    pub family: String,
    pub arch: Option<String>,
    pub min_ram: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}
