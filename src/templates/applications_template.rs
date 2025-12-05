use askama::Template;
use crate::models::{CurrentUser, ApplicationView};

#[derive(Template)]
#[template(path = "applications.html")]
pub struct ApplicationsTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub apps: &'a [ApplicationView],
}
