use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseState {
    pub hostnames: Vec<String>,
    pub region: String,
    pub instance_class: String,
    pub plan_type: String,
    pub assign_ipv4: bool,
    pub assign_ipv6: bool,
    pub floating_ip_count: i32,
    pub ssh_key_ids: Vec<i64>,
    pub os_id: String,
}
