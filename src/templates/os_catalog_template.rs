use askama::Template;
use crate::models::{CurrentUser, OsItem};

#[derive(Template)]
#[template(path = "os.html")]
pub struct OsCatalogTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub os_list: &'a [OsItem],
}
