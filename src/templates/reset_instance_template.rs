use askama::Template;
use crate::models::{CurrentUser, InstanceView};

#[derive(Template)]
#[template(path = "reset_instance.html")]
pub struct ResetInstanceTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub disabled_by_env: bool,
    pub disabled_by_host: bool,
}

crate::impl_base_template!(ResetInstanceTemplate);
