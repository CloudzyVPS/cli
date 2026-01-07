use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "coming_soon.html")]
pub struct ComingSoonTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub feature_name: String,
}

crate::impl_base_template!(ComingSoonTemplate);

