use askama::Template;
use crate::models::{CurrentUser, InstanceView};

#[derive(Template)]
#[template(path = "delete_instance.html")]
pub struct DeleteInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}
