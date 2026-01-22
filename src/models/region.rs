use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Region {
    pub id: String,
    pub name: String,
    // Old fields - kept for backward compatibility
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub city: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    // New OpenAPI-aligned fields
    #[serde(default)]
    pub abbr: String,
    pub image: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub is_out_of_stock: bool,
    pub overall_activeness: Option<bool>,
    pub ddos_activeness: Option<bool>,
    #[serde(default)]
    pub is_premium: bool,
    #[serde(default)]
    pub is_hidden: bool,
    #[serde(default)]
    pub has_offset_price: bool,
    pub max_discount_percent: Option<f64>,
    pub position: Option<i32>,
    pub config: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}
