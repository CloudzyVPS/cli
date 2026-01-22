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
            instance.vcpu_count = data.get("vcpuCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            instance.ram = data.get("ram").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            instance.disk = data.get("disk").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            instance.inserted_at = data.get("insertedAt").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.os_id = data.get("osId").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.iso_id = data.get("isoId").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.from_image = data.get("fromImage").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.user_id = data.get("userId").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.app_id = data.get("appId").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.main_ipv6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.product_id = data.get("productId").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.network_status = data.get("networkStatus").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.discount_percent = data.get("discountPercent").and_then(|v| v.as_i64()).map(|i| i as i32);
            instance.attach_iso = data.get("attachIso").and_then(|v| v.as_bool());
            instance.class = data.get("class").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.oca_data = data.get("ocaData").cloned();
            instance.is_ddos_protected = data.get("isDdosProtected").and_then(|v| v.as_bool());
            instance.customer_note = data.get("customerNote").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.admin_note = data.get("adminNote").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Parse extra_resource if present
            if let Some(er_obj) = data.get("extraResource").and_then(|v| v.as_object()) {
                use crate::models::instance_view::ExtraResource;
                instance.extra_resource = Some(ExtraResource {
                    cpu: er_obj.get("cpu").and_then(|v| v.as_i64()).map(|i| i as i32),
                    ram_in_gb: er_obj.get("ramInGB").and_then(|v| v.as_i64()).map(|i| i as i32),
                    disk_in_gb: er_obj.get("diskInGB").and_then(|v| v.as_i64()).map(|i| i as i32),
                    bandwidth_in_tb: er_obj.get("bandwidthInTB").and_then(|v| v.as_i64()).map(|i| i as i32),
                });
            }

            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                    is_active: os_obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true),
                });
            }

            // Build display fields
            instance.status_display = crate::utils::format_status(&instance.status);
            instance.vcpu_count_display = if instance.vcpu_count > 0 { instance.vcpu_count.to_string() } else { "—".into() };
            instance.ram_display = if instance.ram > 0 { format!("{} MB", instance.ram) } else { "—".into() };
            instance.disk_display = if instance.disk > 0 { format!("{} GB", instance.disk) } else { "—".into() };
        }
    }
    instance
}
