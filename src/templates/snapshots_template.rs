use askama::Template;
use crate::models::CurrentUser;
use crate::api::SnapshotView;

#[derive(Template)]
#[template(path = "snapshots.html")]
pub struct SnapshotsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub snapshots: &'a [SnapshotView],
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
    pub total_count: usize,
    pub filter_instance_id: Option<String>,
}

crate::impl_base_template!(SnapshotsTemplate<'_>);
