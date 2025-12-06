use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceCheckbox {
    pub id: String,
    pub hostname: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminView {
    pub username: String,
    pub instances: Vec<InstanceCheckbox>,
}
