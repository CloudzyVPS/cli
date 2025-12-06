use crate::models::os_item::OsItem;

#[derive(Clone)]
pub struct InstanceView {
    pub id: String,
    pub hostname: String,
    pub region: String,
    pub status: String,
    pub vcpu_count_display: String,
    pub ram_display: String,
    pub disk_display: String,
    pub main_ip: Option<String>,
    pub os: Option<OsItem>,
}
