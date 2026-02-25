use askama::Template;
use crate::models::{CurrentUser, WorkspaceRecord};

#[derive(Template)]
#[template(path = "workspaces.html")]
pub struct WorkspacesTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub workspaces: &'a [WorkspaceRecord],
}

crate::impl_base_template!(WorkspacesTemplate<'_>);
