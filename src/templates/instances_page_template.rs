use askama::Template;
use crate::models::{CurrentUser, InstanceView};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "instances.html")]
pub struct InstancesPageTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instances: &'a [InstanceView],
}

crate::impl_base_template!(InstancesPageTemplate);
