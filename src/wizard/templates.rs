use askama::Template;
use crate::users::models::CurrentUser;
use crate::api::{Region, ProductView, OsItem};
use crate::wizard::models::*;

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

#[derive(Template)]
#[template(path = "step_2.html")]
pub struct Step2Template<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub base_state: &'a BaseState,
    pub form_data: Step2FormData,
    pub back_url: String,
    pub submit_url: String,
}

#[derive(Template)]
#[template(path = "step_3_fixed.html")]
pub struct Step3FixedTemplate<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub base_state: &'a BaseState,
    pub products: &'a [ProductView],
    pub has_products: bool,
    pub selected_product_id: String,
    pub region_name: String,
    pub floating_ip_count: String,
    pub back_url: String,
    pub submit_url: String,
    pub restart_url: String,
    pub ssh_key_ids_csv: String,
    pub hostnames_csv: String,
}

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

#[derive(Template)]
#[template(path = "step_4.html")]
pub struct Step4Template<'a> {
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
    pub back_url: String,
    pub submit_url: String,
    pub restart_url: String,
}

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

pub struct SshKeyDisplay {
    pub id: i64,
    pub name: String,
}

#[derive(Template)]
#[template(path = "step_6.html")]
pub struct Step6Template<'a> {
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
    pub ssh_keys: &'a [SshKeyDisplay],
    pub selected_key_ids: Vec<i64>,
    pub back_url: String,
    pub submit_url: String,
    pub restart_url: String,
}

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

#[derive(Template)]
#[template(path = "step_8.html")]
pub struct Step8Template {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub success: bool,
    pub message: String,
    pub instances_url: String,
}
