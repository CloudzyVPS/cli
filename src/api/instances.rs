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
            let vcpu_count = obj.get("vcpuCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let ram = obj.get("ram").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let disk = obj.get("disk").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let inserted_at = obj.get("insertedAt").and_then(|v| v.as_str()).map(|s| s.to_string());
            let os_id = obj.get("osId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let iso_id = obj.get("isoId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let from_image = obj.get("fromImage").and_then(|v| v.as_str()).map(|s| s.to_string());
            let region = obj.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let user_id = obj.get("userId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let app_id = obj.get("appId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let main_ip = obj.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            let main_ipv6 = obj.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            let product_id = obj.get("productId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let network_status = obj.get("networkStatus").and_then(|v| v.as_str()).map(|s| s.to_string());
            let discount_percent = obj.get("discountPercent").and_then(|v| v.as_i64()).map(|i| i as i32);
            let attach_iso = obj.get("attachIso").and_then(|v| v.as_bool());
            let class = obj.get("class").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let oca_data = obj.get("ocaData").cloned();
            let is_ddos_protected = obj.get("isDdosProtected").and_then(|v| v.as_bool());
            let customer_note = obj.get("customerNote").and_then(|v| v.as_str()).map(|s| s.to_string());
            let admin_note = obj.get("adminNote").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Parse extra_resource if present
            let extra_resource = obj.get("extraResource").and_then(|v| v.as_object()).map(|er| {
                use crate::models::instance_view::ExtraResource;
                ExtraResource {
                    cpu: er.get("cpu").and_then(|v| v.as_i64()).map(|i| i as i32),
                    ram_in_gb: er.get("ramInGB").and_then(|v| v.as_i64()).map(|i| i as i32),
                    disk_in_gb: er.get("diskInGB").and_then(|v| v.as_i64()).map(|i| i as i32),
                    bandwidth_in_tb: er.get("bandwidthInTB").and_then(|v| v.as_i64()).map(|i| i as i32),
                }
            });

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

            // Build display fields
            let status_display = crate::utils::format_status(&status);
            let vcpu_count_display = if vcpu_count > 0 { vcpu_count.to_string() } else { "—".into() };
            let ram_display = if ram > 0 { format!("{} MB", ram) } else { "—".into() };
            let disk_display = if disk > 0 { format!("{} GB", disk) } else { "—".into() };
            
            all_instances.push(InstanceView {
                id,
                hostname,
                vcpu_count,
                ram,
                disk,
                inserted_at,
                os_id,
                iso_id,
                from_image,
                os,
                region,
                user_id,
                app_id,
                status,
                main_ip,
                main_ipv6,
                product_id,
                network_status,
                discount_percent,
                attach_iso,
                extra_resource,
                class,
                oca_data,
                is_ddos_protected,
                customer_note,
                admin_note,
                status_display,
                vcpu_count_display,
                ram_display,
                disk_display,
<<<<<<< copilot/update-ui-templates-responsive-design-again
                vcpu_count,
                ram,
                disk,
                main_ip,
                main_ipv6,
                os,
                product_id,
                network_status,
                extra_resource,
                class,
                is_ddos_protected,
                inserted_at,
=======
>>>>>>> main
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
