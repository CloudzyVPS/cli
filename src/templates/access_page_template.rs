use askama::Template;
use crate::models::{CurrentUser, AdminView};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "access.html")]
pub struct AccessPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub admins: &'a [AdminView],
}

crate::impl_base_template!(AccessPageTemplate);
