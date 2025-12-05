use std::collections::HashMap;
use crate::models::{InstanceView, OsItem, UserRecord};
use super::client::api_call;

/// Load instances for a specific user from the API.
/// Filters instances based on user role and assigned instances.
pub async fn load_instances_for_user(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    users_map: &HashMap<String, UserRecord>,
    username: &str,
) -> Vec<InstanceView> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/instances", None, None).await;
    let mut all_instances = Vec::new();
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let hostname = obj.get("hostname").and_then(|v| v.as_str()).unwrap_or("(no hostname)").to_string();
                    let region = obj.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let vcpu_count_display = obj.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
                    let ram_display = obj.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
                    let disk_display = obj.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
                    let main_ip = obj.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
                    
                    let os = if let Some(os_obj) = obj.get("os").and_then(|v| v.as_object()) {
                        Some(OsItem {
                            id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                        })
                    } else {
                        None
                    };
                    
                    all_instances.push(InstanceView {
                        id,
                        hostname,
                        region,
                        status,
                        vcpu_count_display,
                        ram_display,
                        disk_display,
                        main_ip,
                        os,
                    });
                }
            }
        }
    }
    
    if username.is_empty() {
        return all_instances;
    }
    
    let uname = username.to_lowercase();
    if let Some(user_rec) = users_map.get(&uname) {
        if user_rec.role == "owner" {
            return all_instances;
        }
        return all_instances.into_iter().filter(|inst| user_rec.assigned_instances.contains(&inst.id)).collect();
    }
    
    vec![]
}
