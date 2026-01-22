use serde::Deserialize;

/// Form data for Step 7 (SSH key selection) in the instance creation wizard
/// 
/// This struct is used to deserialize form data when users navigate through
/// the multi-step instance creation process. All fields are preserved to
/// maintain state between wizard steps.
#[derive(Deserialize)]
pub struct SshKeySelectionFormStep7 {
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
