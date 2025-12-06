use askama::Template;
use crate::models::{CurrentUser, UserRow};

#[derive(Template)]
#[template(path = "users.html")]
pub struct UsersPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub rows: &'a [UserRow],
}

crate::impl_base_template!(UsersPageTemplate<'_>);
