use crate::models::os_item::OsItem;

#[derive(Clone, Debug)]
pub struct InstanceView {
    pub id: String,
    pub hostname: String,
    pub region: String,
    pub status: String,
    pub status_display: String,
    pub vcpu_count_display: String,
    pub ram_display: String,
    pub disk_display: String,
    pub main_ip: Option<String>,
    pub main_ipv6: Option<String>,
    pub os: Option<OsItem>,
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
            os: None,
        }
    }
}
