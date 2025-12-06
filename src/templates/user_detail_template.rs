use askama::Template;
use crate::models::{CurrentUser, UserRow};

#[derive(Template)]
#[template(path = "user_detail.html")]
pub struct UserDetailTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub user: UserRow,
}

crate::impl_base_template!(UserDetailTemplate);
