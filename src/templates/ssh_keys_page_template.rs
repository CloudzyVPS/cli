use askama::Template;
use crate::models::{CurrentUser, SshKeyView};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "ssh_keys.html")]
pub struct SshKeysPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub ssh_keys: &'a [SshKeyView],
    pub customer_id: Option<String>,
}

crate::impl_base_template!(SshKeysPageTemplate);
