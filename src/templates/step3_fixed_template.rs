use askama::Template;
use crate::models::{CurrentUser, ProductView, BaseState};

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

crate::impl_base_template!(Step3FixedTemplate<'_>);
