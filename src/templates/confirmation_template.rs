use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "confirm.html")]
pub struct ConfirmationTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    
    pub title: String,
    pub message: String,
    pub target_url: String,
    pub confirm_label: String,
    pub cancel_url: String,
    pub button_class: String,
    pub hidden_fields: Vec<(String, String)>,
}

crate::impl_base_template!(ConfirmationTemplate);

