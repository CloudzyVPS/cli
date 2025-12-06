use serde::{Deserialize, Serialize};

use crate::models::product_entry::ProductEntry;

#[derive(Serialize, Deserialize, Clone)]
pub struct ProductView {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub tags: String,
    pub spec_entries: Vec<ProductEntry>,
    pub price_entries: Vec<ProductEntry>,
    pub cpu: Option<String>,
    pub ram: Option<String>,
    pub storage: Option<String>,
    pub bandwidth: Option<String>,
}
