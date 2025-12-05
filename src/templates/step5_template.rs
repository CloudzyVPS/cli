use askama::Template;
use crate::models::{CurrentUser, BaseState, Step1FormData, Step2FormData, CustomPlanFormValues, Region, ProductView, OsItem};

#[derive(Template)]
#[template(path = "step_5.html")]
pub struct Step5Template<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub base_state: &'a BaseState,
    pub product_id: String,
    pub cpu: String,
    pub ram_in_gb: String,
    pub disk_in_gb: String,
    pub bandwidth_in_tb: String,
    pub os_list: &'a [OsItem],
    pub selected_os_id: String,
    pub back_url: String,
    pub submit_url: String,
    pub restart_url: String,
}
