use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "instance_detail.html")]
pub struct InstanceDetailTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance_id: String,
    pub hostname: String,
    pub details: Vec<(String, String)>,
    pub is_disabled: bool,
}
