use askama::Template;
use crate::models::CurrentUser;

#[derive(Template)]
#[template(path = "clocked_instances.html")]
pub struct ClockedInstancesTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub clocked_ids: &'a [String],
}

crate::impl_base_template!(ClockedInstancesTemplate<'_>);
