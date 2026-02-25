use askama::Template;
use crate::models::{CurrentUser, WorkspaceRecord};

#[derive(Template)]
#[template(path = "workspace_detail.html")]
pub struct WorkspaceDetailTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub workspace: &'a WorkspaceRecord,
    pub all_users: &'a [String],
}

crate::impl_base_template!(WorkspaceDetailTemplate<'_>);
