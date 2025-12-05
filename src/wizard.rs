use askama::Template;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::users::CurrentUser;
use crate::api::{Region, ProductView, OsItem};
use crate::util::{parse_flag, parse_optional_int, parse_int_list, build_query_string};
use urlencoding::encode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseState {
    pub hostnames: Vec<String>,
    pub region: String,
    pub instance_class: String,
    pub plan_type: String,
    pub assign_ipv4: bool,
    pub assign_ipv6: bool,
    pub floating_ip_count: i32,
    pub ssh_key_ids: Vec<i64>,
    pub os_id: String,
}

pub fn parse_wizard_base(query: &HashMap<String, String>) -> BaseState {
    let mut hostnames: Vec<String> = query
        .get("hostnames")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    hostnames.retain(|h| !h.is_empty());
    let region = query
        .get("region")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let instance_class = query
        .get("instance_class")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".into());
    let plan_type = query
        .get("plan_type")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| matches!(s.as_str(), "fixed" | "custom"))
        .unwrap_or_else(|| "fixed".into());
    let assign_ipv4 = parse_flag(query.get("assign_ipv4"), true);
    let assign_ipv6 = parse_flag(query.get("assign_ipv6"), false);
    let floating_ip_count = parse_optional_int(query.get("floating_ip_count")).unwrap_or(0);
    let ssh_raw = query
        .get("ssh_key_ids")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ssh_key_ids = parse_int_list(&ssh_raw);
    let os_id = query
        .get("os_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    BaseState {
        hostnames,
        region,
        instance_class,
        plan_type,
        assign_ipv4,
        assign_ipv6,
        floating_ip_count,
        ssh_key_ids,
        os_id,
    }
}

pub fn build_base_query_pairs(state: &BaseState) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for h in &state.hostnames {
        pairs.push(("hostnames".into(), h.clone()));
    }
    if !state.region.is_empty() {
        pairs.push(("region".into(), state.region.clone()));
    }
    pairs.push(("instance_class".into(), state.instance_class.clone()));
    pairs.push(("plan_type".into(), state.plan_type.clone()));
    pairs.push(("assign_ipv4".into(), (state.assign_ipv4 as u8).to_string()));
    pairs.push(("assign_ipv6".into(), (state.assign_ipv6 as u8).to_string()));
    if state.floating_ip_count > 0 {
        pairs.push((
            "floating_ip_count".into(),
            state.floating_ip_count.to_string(),
        ));
    }
    for id in &state.ssh_key_ids {
        pairs.push(("ssh_key_ids".into(), id.to_string()));
    }
    if !state.os_id.is_empty() {
        pairs.push(("os_id".into(), state.os_id.clone()));
    }
    pairs
}

#[derive(Clone, Default)]
pub struct Step1FormData {
    pub region: String,
    pub instance_class: String,
    pub plan_type: String,
}

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

#[derive(Clone)]
pub struct Step2FormData {
    pub hostnames_text: String,
    pub assign_ipv4: bool,
    pub assign_ipv6: bool,
    pub floating_ip_count: String,
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

#[derive(Clone)]
pub struct CustomPlanFormValues {
    pub cpu: String,
    pub ram_in_gb: String,
    pub disk_in_gb: String,
    pub bandwidth_in_tb: String,
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

pub struct SshKeyDisplay {
    pub id: i64,
    pub name: String,
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

#[derive(Deserialize)]
pub struct Step7Form {
    pub product_id: Option<String>,
    pub cpu: Option<String>,
    #[serde(rename = "ramInGB")]
    pub ram_in_gb: Option<String>,
    #[serde(rename = "diskInGB")]
    pub disk_in_gb: Option<String>,
    #[serde(rename = "bandwidthInTB")]
    pub bandwidth_in_tb: Option<String>,
    pub region: String,
    pub os_id: String,
    pub ssh_key_ids: Option<String>,
    pub hostnames: String,
    pub assign_ipv4: Option<String>,
    pub assign_ipv6: Option<String>,
    pub floating_ip_count: Option<String>,
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
