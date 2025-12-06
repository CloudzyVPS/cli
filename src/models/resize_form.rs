use serde::Deserialize;

#[derive(Deserialize)]
pub struct ResizeForm {
    pub r#type: String,
    pub product_id: Option<String>,
    pub region_id: Option<String>,
    pub cpu: Option<String>,
    #[serde(rename = "ramInGB")]
    pub ram_in_gb: Option<String>,
    #[serde(rename = "diskInGB")]
    pub disk_in_gb: Option<String>,
    #[serde(rename = "bandwidthInTB")]
    pub bandwidth_in_tb: Option<String>,
}
