use askama::Template;
use crate::models::{CurrentUser, SshKeyView};

#[derive(Template)]
#[template(path = "ssh_key_detail.html")]
pub struct SshKeyDetailTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub ssh_key: Option<SshKeyView>,
    pub key_id: String,
}

crate::impl_base_template!(SshKeyDetailTemplate);
