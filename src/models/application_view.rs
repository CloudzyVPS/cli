use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ApplicationView {
    pub id: String,
    pub name: String,
    pub short_description: String,
    pub description: String,
    pub support_level: String,
    pub price: Option<String>,
    pub tags: Option<String>,
}
