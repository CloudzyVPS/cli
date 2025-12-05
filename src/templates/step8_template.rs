use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "step_8.html")]
pub struct Step8Template {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub status_label: String,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub errors: Vec<String>,
    pub back_url: String,
}

crate::impl_base_template!(Step8Template);
