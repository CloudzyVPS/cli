use askama::Template;
use crate::models::CurrentUser;
use serde_json::Map;

#[derive(Template)]
#[template(path = "snapshot_detail.html")]
pub struct SnapshotDetailTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub snapshot_id: String,
    pub snapshot_data: Option<Map<String, serde_json::Value>>,
}

crate::impl_base_template!(SnapshotDetailTemplate);
