use serde::{Deserialize, Serialize};

use crate::models::product_entry::ProductEntry;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlanSpecification {
    pub cpu: f64,
    pub ram: f64,
    pub ram_in_mb: f64,
    pub storage: f64,
    pub bandwidth_in_tb: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Plan {
    pub id: String,
    #[serde(rename = "type")]
    pub plan_type: Option<String>,
    pub gpu_name: Option<String>,
    pub gpu_quantity: Option<i32>,
    pub specification: PlanSpecification,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PriceItem {
    pub name: String,
    pub hourly_price: f64,
    pub monthly_price: f64,
    pub hourly_price_without_discount: f64,
    pub monthly_price_without_discount: f64,
    pub discount_percent: i32,
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProductView {
    pub id: String,
    pub region_id: String,
    pub plan_id: String,
    pub is_active: bool,
    pub network_max_rate: f64,
    pub network_max_rate95: f64,
    pub discount_percent: i32,
    pub remaining_actual_stock: Option<i32>,
    pub remaining_preorder_capacity: Option<i32>,
    pub plan: Plan,
    pub overall_activeness: bool,
    pub ddos_activeness: Option<bool>,
    pub price_items: Vec<PriceItem>,
    
    // Display/helper fields (not from API, used by templates)
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub display_name: String,
    #[serde(skip)]
    pub description: String,
    #[serde(skip)]
    pub tags: String,
    #[serde(skip)]
    pub spec_entries: Vec<ProductEntry>,
    #[serde(skip)]
    pub price_entries: Vec<ProductEntry>,
    #[serde(skip)]
    pub cpu: Option<String>,
    #[serde(skip)]
    pub ram: Option<String>,
    #[serde(skip)]
    pub storage: Option<String>,
    #[serde(skip)]
    pub bandwidth: Option<String>,
}
