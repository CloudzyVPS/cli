use crate::models::os_item::OsItem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraResource {
    pub cpu: Option<i32>,
    pub ram_in_gb: Option<i32>,
    pub disk_in_gb: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct InstanceView {
    pub id: String,
    pub hostname: String,
    pub region: String,
    pub status: String,
    // Display fields (computed)
    pub status_display: String,
    pub vcpu_count_display: String,
    pub ram_display: String,
    pub disk_display: String,
    // Raw data fields (new OpenAPI-aligned)
    pub vcpu_count: Option<i32>,
    pub ram: Option<i32>,
    pub disk: Option<i32>,
    pub main_ip: Option<String>,
    pub main_ipv6: Option<String>,
    pub os: Option<OsItem>,
    pub product_id: Option<String>,
    pub network_status: Option<String>,
    pub extra_resource: Option<ExtraResource>,
    pub class: Option<String>,
    pub is_ddos_protected: Option<bool>,
    pub inserted_at: Option<String>,
}

impl InstanceView {
    /// Creates a new InstanceView with default values for the given instance ID.
    pub fn new_with_defaults(instance_id: String) -> Self {
        Self {
            id: instance_id,
            hostname: "(no hostname)".into(),
            region: "".into(),
            main_ip: None,
            main_ipv6: None,
            status: "".into(),
            status_display: "".into(),
            vcpu_count_display: "—".into(),
            ram_display: "—".into(),
            disk_display: "—".into(),
            vcpu_count: None,
            ram: None,
            disk: None,
            os: None,
            product_id: None,
            network_status: None,
            extra_resource: None,
            class: None,
            is_ddos_protected: None,
            inserted_at: None,
        }
    }
}
