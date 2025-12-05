use askama::Template;
use crate::models::{CurrentUser, BaseState, Step1FormData, Step2FormData, CustomPlanFormValues, Region, ProductView, OsItem};

#[derive(Template)]
#[template(path = "step_7.html")]
pub struct Step7Template {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub product_id: String,
    pub cpu: String,
    pub ram_in_gb: String,
    pub disk_in_gb: String,
    pub bandwidth_in_tb: String,
    pub region: String,
    pub os_id: String,
    pub ssh_key_ids_csv: String,
    pub hostnames_csv: String,
    pub assign_ipv4: bool,
    pub assign_ipv6: bool,
    pub floating_ip_count: i32,
    pub back_url: String,
}
