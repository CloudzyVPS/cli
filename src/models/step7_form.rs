use serde::Deserialize;

#[derive(Deserialize)]
pub struct Step7Form {
    pub product_id: Option<String>,
    pub cpu: Option<String>,
    #[serde(rename = "ramInGB")]
    pub ram_in_gb: Option<String>,
    #[serde(rename = "diskInGB")]
    pub disk_in_gb: Option<String>,
    #[serde(rename = "bandwidthInTB")]
    pub bandwidth_in_tb: Option<String>,
    pub region: String,
    pub os_id: String,
    pub ssh_key_ids: Option<String>,
    pub hostnames: String,
    pub assign_ipv4: Option<String>,
    pub assign_ipv6: Option<String>,
    pub floating_ip_count: Option<String>,
}
