use askama::Template;
use crate::models::{CurrentUser, Region, InstanceView};

#[derive(Template)]
#[template(path = "resize.html")]
pub struct ResizeTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub instance: InstanceView,
    pub regions: &'a [Region],
    pub disabled_by_env: bool,
    pub disabled_by_host: bool,
}

crate::impl_base_template!(ResizeTemplate<'_>);
