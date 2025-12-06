use askama::Template;
use crate::models::{CurrentUser, BaseState, CustomPlanFormValues, SshKeyDisplay};

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
    pub hostnames_csv: String,
    pub floating_ip_count: String,
    pub extra_disk: String,
    pub extra_bandwidth: String,
    pub custom_plan: CustomPlanFormValues,
    pub ssh_keys: &'a [SshKeyDisplay],
    pub manage_keys_url: String,
    pub back_url: String,
    pub submit_url: String,
}

crate::impl_base_template!(Step6Template<'_>);
