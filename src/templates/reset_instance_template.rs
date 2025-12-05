use askama::Template;
use crate::models::{CurrentUser, Region, ProductView, OsItem, ApplicationView, InstanceView};

#[derive(Template)]
#[template(path = "reset_instance.html")]
pub struct ResetInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}
