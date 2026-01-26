use askama::Template;
use crate::models::{CurrentUser, Region};
use crate::api::IsoView;

#[derive(Template)]
#[template(path = "isos.html")]
pub struct IsosTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub isos: &'a [IsoView],
    pub regions: &'a [Region],
    pub total_count: usize,
}

crate::impl_base_template!(IsosTemplate<'_>);
