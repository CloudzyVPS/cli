use crate::models::os_item::OsItem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraResource {
    pub cpu: Option<i32>,
    pub ram_in_gb: Option<i32>,
    pub disk_in_gb: Option<i32>,
    pub bandwidth_in_tb: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceView {
    pub id: String,
    pub hostname: String,
    pub vcpu_count: i32,
    pub ram: i32,
    pub disk: i32,
    pub inserted_at: Option<String>,
    pub os_id: Option<String>,
    pub iso_id: Option<String>,
    pub from_image: Option<String>,
    pub os: Option<OsItem>,
    pub region: String,
    pub user_id: Option<String>,
    pub app_id: Option<String>,
    pub status: String,
    pub main_ip: Option<String>,
    pub main_ipv6: Option<String>,
    pub product_id: Option<String>,
    pub network_status: Option<String>,
    pub discount_percent: Option<i32>,
    pub attach_iso: Option<bool>,
    pub extra_resource: Option<ExtraResource>,
    pub class: String,
    pub oca_data: Option<serde_json::Value>,
    pub is_ddos_protected: Option<bool>,
    pub customer_note: Option<String>,
    pub admin_note: Option<String>,
    
    // Display helpers (not from API)
    #[serde(skip)]
    pub status_display: String,
    #[serde(skip)]
    pub vcpu_count_display: String,
    #[serde(skip)]
    pub ram_display: String,
    #[serde(skip)]
    pub disk_display: String,
}

impl InstanceView {
    /// Creates a new InstanceView with default values for the given instance ID.
    pub fn new_with_defaults(instance_id: String) -> Self {
        Self {
            id: instance_id,
            hostname: "(no hostname)".into(),
            vcpu_count: 0,
            ram: 0,
            disk: 0,
            inserted_at: None,
            os_id: None,
            iso_id: None,
            from_image: None,
            os: None,
            region: "".into(),
            user_id: None,
            app_id: None,
            status: "".into(),
            main_ip: None,
            main_ipv6: None,
            product_id: None,
            network_status: None,
            discount_percent: None,
            attach_iso: None,
            extra_resource: None,
            class: "".into(),
            oca_data: None,
            is_ddos_protected: None,
            customer_note: None,
            admin_note: None,
            status_display: "".into(),
            vcpu_count_display: "—".into(),
            ram_display: "—".into(),
            disk_display: "—".into(),
        }
    }
}
