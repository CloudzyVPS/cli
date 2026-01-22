use std::collections::HashMap;
use crate::models::{InstanceView, OsItem, UserRecord};
use super::client::api_call;

/// Paginated result structure for instances
#[derive(Clone, Debug)]
pub struct PaginatedInstances {
    pub instances: Vec<InstanceView>,
    pub total_count: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

/// Load instances for a specific user from the API with pagination support.
/// Filters instances based on user role and assigned instances.
/// 
/// # Parameters
/// - `page`: Page number (1-indexed). Use 0 to disable pagination and return all instances.
/// - `per_page`: Number of items per page. Default is 20.
pub async fn load_instances_for_user(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    users_map: &HashMap<String, UserRecord>,
    username: &str,
    page: usize,
    per_page: usize,
) -> PaginatedInstances {
    let mut all_instances_data = Vec::new();
    let mut current_bookmark: Option<String> = None;

    loop {
        let mut params = Vec::new();
        params.push(("limit".to_string(), "100".to_string()));
        if let Some(ref b) = current_bookmark {
            params.push(("bookmark".to_string(), b.clone()));
        }

        let payload = api_call(client, api_base_url, api_token, "GET", "/v1/instances", None, Some(params)).await;
        
        if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
                let instances_arr = data.get("instances").and_then(|i| i.as_array());
                
                if let Some(arr) = instances_arr {
                    if arr.is_empty() {
                        break;
                    }
                    all_instances_data.extend(arr.clone());
                }

                // Check for next bookmark
                let next_bookmark = data.get("bookmark").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                // If no bookmark, or it's the same as the one we just used, or we got no instances, break
                if next_bookmark.is_none() || next_bookmark == current_bookmark || instances_arr.map_or(true, |a| a.is_empty()) {
                    break;
                }
                current_bookmark = next_bookmark;
            } else if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
                // Fallback for older API versions that might return array directly
                all_instances_data.extend(arr.clone());
                break;
            } else {
                break;
            }
        } else {
            break;
        }

        // Limit to prevent infinite loops if something goes wrong
        if all_instances_data.len() > 5000 {
            break;
        }
    }

    let mut all_instances = Vec::new();
    for item in all_instances_data {
        if let Some(obj) = item.as_object() {
            let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let hostname = obj.get("hostname").and_then(|v| v.as_str()).unwrap_or("(no hostname)").to_string();
            let region = obj.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let status_display = crate::utils::format_status(&status);
            let vcpu_count_display = obj.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            let ram_display = obj.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
            let disk_display = obj.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
            let main_ip = obj.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            let main_ipv6 = obj.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            
            let os = if let Some(os_obj) = obj.get("os").and_then(|v| v.as_object()) {
                Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                    is_active: os_obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true),
                })
            } else {
                None
            };
            
            all_instances.push(InstanceView {
                id,
                hostname,
                region,
                status,
                status_display,
                vcpu_count_display,
                ram_display,
                disk_display,
                main_ip,
                main_ipv6,
                os,
            });
        }
    }
    
    // Filter instances based on user permissions
    let filtered_instances = if username.is_empty() {
        all_instances
    } else {
        let uname = username.to_lowercase();
        if let Some(user_rec) = users_map.get(&uname) {
            if user_rec.role == "owner" {
                all_instances
            } else {
                all_instances.into_iter().filter(|inst| user_rec.assigned_instances.contains(&inst.id)).collect()
            }
        } else {
            vec![]
        }
    };
    
    let total_count = filtered_instances.len();
    
    // If page is 0 or per_page is 0, return all instances without pagination
    if page == 0 || per_page == 0 {
        return PaginatedInstances {
            instances: filtered_instances,
            total_count,
            current_page: 0,
            total_pages: 1,
            per_page: total_count,
        };
    }
    
    // Calculate pagination
    let total_pages = if total_count == 0 {
        1
    } else {
        (total_count + per_page - 1) / per_page
    };
    
    // Clamp page to valid range
    let current_page = page.max(1).min(total_pages);
    
    // Calculate slice range
    let start_idx = (current_page - 1) * per_page;
    let end_idx = (start_idx + per_page).min(total_count);
    
    let paginated_instances = if start_idx < total_count {
        filtered_instances[start_idx..end_idx].to_vec()
    } else {
        vec![]
    };
    
    PaginatedInstances {
        instances: paginated_instances,
        total_count,
        current_page,
        total_pages,
        per_page,
    }
}
