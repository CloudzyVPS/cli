use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub version: &'static str,
    pub latest_version: Option<String>,
}

crate::impl_base_template!(AboutTemplate);

