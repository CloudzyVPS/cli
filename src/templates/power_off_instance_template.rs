use askama::Template;
use crate::models::{CurrentUser, InstanceView};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "poweroff_instance.html")]
pub struct PowerOffInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

crate::impl_base_template!(PowerOffInstanceTemplate);
