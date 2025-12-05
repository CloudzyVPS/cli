use askama::Template;
use crate::models::{CurrentUser, Step1FormData, Region};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "step_1.html")]
pub struct Step1Template<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub regions: &'a [Region],
    pub form_data: Step1FormData,
}

crate::impl_base_template!(Step1Template);
