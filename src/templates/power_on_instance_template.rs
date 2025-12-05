use askama::Template;
use crate::models::{CurrentUser, InstanceView};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "poweron_instance.html")]
pub struct PowerOnInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub is_disabled: bool,
}

crate::impl_base_template!(PowerOnInstanceTemplate);
