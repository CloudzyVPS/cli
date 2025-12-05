use askama::Template;
use crate::models::{CurrentUser, BaseState, Step1FormData, Step2FormData, CustomPlanFormValues, Region, ProductView, OsItem};

#[derive(Template)]
#[template(path = "step_3_custom.html")]
pub struct Step3CustomTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub base_state: &'a BaseState,
    pub region_name: String,
    pub floating_ip_count: String,
    pub back_url: String,
    pub submit_url: String,
    pub requirements: Vec<String>,
    pub minimum_ram: i32,
    pub minimum_disk: i32,
    pub form_values: CustomPlanFormValues,
    pub ssh_key_ids_csv: String,
    pub hostnames_csv: String,
}
