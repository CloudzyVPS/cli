use askama::Template;
use crate::models::{CurrentUser, BaseState, Step1FormData, Step2FormData, CustomPlanFormValues, Region, ProductView, OsItem};

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
