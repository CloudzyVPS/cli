use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Region {
    pub id: String,
    pub name: String,
    // New OpenAPI-aligned fields
    pub abbr: Option<String>,
    pub image: Option<String>,
    pub is_active: Option<bool>,
    pub is_out_of_stock: Option<bool>,
    pub overall_activeness: Option<bool>,
    pub ddos_activeness: Option<bool>,
    pub is_premium: Option<bool>,
    pub is_hidden: Option<bool>,
    pub has_offset_price: Option<bool>,
    pub max_discount_percent: Option<f64>,
    pub position: Option<i32>,
    // config could be a generic JSON value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    // Legacy fields (kept for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}
