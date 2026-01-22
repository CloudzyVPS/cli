use serde_json::Value;

use crate::models::{AppState, InstanceView, OsItem};

pub async fn simple_instance_action(state: &AppState, action: &str, instance_id: &str) -> Value {
    let endpoint = format!("/v1/instances/{}/{}", instance_id, action);
    crate::api::api_call(&state.client, &state.api_base_url, &state.api_token, "POST", &endpoint, None, None).await
}

pub enum BlockReason {
    Blacklisted,
    HostnameMatch(String),
}

impl BlockReason {
    pub fn message(&self) -> String {
        match self {
            BlockReason::Blacklisted => "Actions are disabled for this instance.".into(),
            BlockReason::HostnameMatch(h) => format!("Actions are disabled because the instance hostname ({}) matches the hostname of this application server.", h),
        }
    }
}

pub async fn check_instance_block(state: &AppState, instance_id: &str, hostname: Option<&str>) -> Option<BlockReason> {
    if state.is_instance_disabled(instance_id) {
        return Some(BlockReason::Blacklisted);
    }
    
    if let Some(h) = hostname {
        if state.is_hostname_blocked(h) {
            return Some(BlockReason::HostnameMatch(h.to_string()));
        }
    } else {
        // Fetch hostname if not provided
        let instance = get_instance_for_action(state, instance_id).await;
        if state.is_hostname_blocked(&instance.hostname) {
            return Some(BlockReason::HostnameMatch(instance.hostname));
        }
    }
    
    None
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

#[allow(dead_code)]
pub async fn get_instance_for_action(state: &AppState, instance_id: &str) -> InstanceView {
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = crate::api::api_call(&state.client, &state.api_base_url, &state.api_token, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView::new_with_defaults(instance_id.to_string());
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
                    is_active: os_obj.get("isActive").and_then(|v| v.as_bool()),
                });
            }
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.status_display = crate::utils::format_status(&instance.status);
        }
    }
    instance
}
