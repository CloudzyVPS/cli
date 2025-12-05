use askama::Template;
use crate::models::{CurrentUser, Region, ProductView, OsItem, ApplicationView, InstanceView};

#[derive(Template)]
#[template(path = "change_pass_instance.html")]
pub struct ChangePassInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub new_password: Option<String>,
    pub is_disabled: bool,
}
