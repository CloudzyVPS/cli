use askama::Template;
use crate::models::{CurrentUser, BaseState, PlanState, ProductEntry};
use crate::templates::BaseTemplate;

#[derive(Template)]
#[template(path = "step_7.html")]
pub struct Step7Template<'a> {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub base_state: &'a BaseState,
    pub plan_state: PlanState,
    pub hostnames_csv: String,
    pub floating_ip_count: String,
    pub ssh_key_ids_csv: String,
    pub region_name: String,
    pub plan_type_label: String,
    pub hostnames_display: String,
    pub has_plan_summary: bool,
    pub plan_summary: Vec<ProductEntry>,
    pub has_price_entries: bool,
    pub price_entries: Vec<ProductEntry>,
    pub selected_os_label: String,
    pub ssh_keys_display: String,
    pub has_footnote: bool,
    pub footnote_text: String,
    pub back_url: String,
    pub submit_url: String,
}

crate::impl_base_template!(Step7Template);
