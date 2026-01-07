use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsItem {
    pub id: String,
    pub name: String,
    pub family: String,
    pub arch: Option<String>,
    pub min_ram: Option<String>,
    pub is_default: bool,
}
