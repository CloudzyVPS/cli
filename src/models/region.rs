use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegionConfig {
    pub support_ipv6: bool,
    pub support_regular_cpu: bool,
    pub support_high_frequency_cpu: bool,
    pub support_monitoring: bool,
    pub support_gpu: bool,
    pub support_custom_plan: bool,
    pub ram_threshold_in_gb: i32,
    pub ip_threshold: i32,
    pub disk_threshold_in_gb: i32,
    pub support_ddos_ipv4: Option<bool>,
    pub ddos_ipv4_threshold: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Region {
    pub id: String,
    pub name: String,
    pub abbr: String,
    pub image: String,
    pub is_active: bool,
    pub is_out_of_stock: bool,
    pub overall_activeness: bool,
    pub ddos_activeness: Option<bool>,
    pub is_premium: bool,
    pub is_hidden: bool,
    pub has_offset_price: bool,
    pub max_discount_percent: Option<i32>,
    pub position: serde_json::Value, // HashMap<String, i32> in practice
    pub config: RegionConfig,
}
