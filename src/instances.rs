use axum::response::{IntoResponse, Redirect};
use serde::Deserialize;
use serde_json::Value;

use crate::api::{api_call, load_os_list, load_products, load_regions, InstanceView, OsItem};

pub struct AppState {
    pub users: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, crate::users::UserRecord>>>,
    pub sessions: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    pub flash_store: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
    pub default_customer_cache: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub api_base_url: String,
    pub api_token: String,
    pub public_base_url: String,
    pub client: reqwest::Client,
    pub disabled_instances: std::sync::Arc<std::collections::HashSet<String>>,
}

impl AppState {
    pub fn is_instance_disabled(&self, id: &str) -> bool {
        self.disabled_instances.contains(id)
    }
}

pub async fn simple_instance_action(state: &AppState, action: &str, instance_id: &str) -> Value {
    let endpoint = format!("/v1/instances/{}/{}", instance_id, action);
    api_call(&state.client, &state.api_base_url, &state.api_token, "POST", &endpoint, None, None).await
}

pub async fn enforce_instance_access(state: &AppState, username: Option<&str>, instance_id: &str) -> bool {
    if let Some(username) = username {
        let users = state.users.lock().unwrap();
        if let Some(rec) = users.get(username) {
            if rec.role == "owner" {
                return true;
            }
            return rec.assigned_instances.iter().any(|id| id == instance_id);
        }
    }
    false
}

#[derive(Deserialize)]
pub struct AddTrafficForm {
    pub traffic_amount: String,
}

#[derive(Deserialize)]
pub struct ChangeOsForm {
    pub os_id: String,
}

#[derive(Deserialize)]
pub struct ResizeForm {
    pub r#type: String,
    pub product_id: Option<String>,
    pub region_id: Option<String>,
    pub cpu: Option<String>,
    #[serde(rename = "ramInGB")]
    pub ram_in_gb: Option<String>,
    #[serde(rename = "diskInGB")]
    pub disk_in_gb: Option<String>,
    #[serde(rename = "bandwidthInTB")]
    pub bandwidth_in_tb: Option<String>,
}

pub async fn get_instance_for_action(state: &AppState, instance_id: &str) -> InstanceView {
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call(&state.client, &state.api_base_url, &state.api_token, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView {
        id: instance_id.to_string(),
        hostname: "(no hostname)".into(),
        region: "".into(),
        main_ip: None,
        status: "".into(),
        vcpu_count_display: "—".into(),
        ram_display: "—".into(),
        disk_display: "—".into(),
        os: None,
    };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.vcpu_count_display = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            instance.ram_display = data.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
            instance.disk_display = data.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    instance
}
