use serde_json::Value;
use std::collections::HashMap;

use crate::models::{Region, ProductEntry, ProductView, OsItem, ApplicationView, InstanceView, UserRecord};
use crate::utils::value_to_short_string;
pub async fn api_call(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    method: &str,
    endpoint: &str,
    body: Option<Value>,
    params: Option<Vec<(String, String)>>,
) -> Value {
    let url = format!("{}{}", api_base_url, endpoint);
    let mut req = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };
    
    if !api_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_token));
    }
    
    if let Some(p) = params {
        req = req.query(&p);
    }
    
    if let Some(b) = body {
        req = req.json(&b);
    }
    
    match req.send().await {
        Ok(resp) => resp.json().await.unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse response"})),
        Err(e) => serde_json::json!({"error": format!("Request failed: {}", e)}),
    }
}

pub async fn load_regions(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> (Vec<Region>, HashMap<String, Region>) {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/regions", None, None).await;
    let mut regions = Vec::new();
    let mut map = HashMap::new();
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for r in arr {
                if let Some(obj) = r.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string();
                    let slug = obj
                        .get("slug")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let country = obj
                        .get("country")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let city = obj
                        .get("city")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let latitude = obj.get("latitude").and_then(|v| v.as_f64());
                    let longitude = obj.get("longitude").and_then(|v| v.as_f64());

                    let region = Region {
                        id: id.clone(),
                        name,
                        slug,
                        country,
                        city,
                        latitude,
                        longitude,
                    };
                    regions.push(region.clone());
                    map.insert(id, region);
                }
            }
        }
    }
    (regions, map)
}

pub async fn load_products(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    region_id: &str,
) -> Vec<ProductView> {
    let params = vec![("regionId".into(), region_id.to_string())];
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/products", None, Some(params)).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    let plan = obj.get("plan").and_then(|v| v.as_object());
                    let price_items = obj.get("priceItems").and_then(|v| v.as_array());

                    let name = id.clone();
                    
                    let display_name = name.clone();
                    let description = obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tags = obj
                        .get("tags")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(); 

                    let mut spec_entries = Vec::new();
                    let mut cpu = None;
                    let mut ram = None;
                    let mut storage = None;
                    let mut bandwidth = None;

                    if let Some(p) = plan {
                        if let Some(spec) = p.get("specification").and_then(|v| v.as_object()) {
                            if let Some(c) = spec.get("cpu") {
                                let val = value_to_short_string(c);
                                cpu = Some(format!("{} vCPU", val));
                                spec_entries.push(ProductEntry { term: "CPU".into(), value: format!("{} vCPU", val) });
                            }
                            if let Some(r) = spec.get("ram") {
                                let val = value_to_short_string(r);
                                ram = Some(format!("{} GB", val));
                                spec_entries.push(ProductEntry { term: "RAM".into(), value: format!("{} GB", val) });
                            }
                            if let Some(s) = spec.get("storage") {
                                let val = value_to_short_string(s);
                                storage = Some(format!("{} GB", val));
                                spec_entries.push(ProductEntry { term: "Storage".into(), value: format!("{} GB", val) });
                            }
                            if let Some(b) = spec.get("bandwidthInTB") {
                                let val = value_to_short_string(b);
                                bandwidth = Some(format!("{} TB", val));
                                spec_entries.push(ProductEntry { term: "Bandwidth".into(), value: format!("{} TB", val) });
                            }
                        }
                    }

                    let mut price_entries = Vec::new();
                    if let Some(items) = price_items {
                        for item in items {
                            if let Some(monthly) = item.get("monthlyPrice") {
                                price_entries.push(ProductEntry { term: "Monthly".into(), value: format!("${}", value_to_short_string(monthly)) });
                            }
                        }
                    }

                    out.push(ProductView {
                        id,
                        name,
                        display_name,
                        description,
                        tags,
                        spec_entries,
                        price_entries,
                        cpu,
                        ram,
                        storage,
                        bandwidth,
                    });
                }
            }
        }
    }
    out
}

pub async fn load_os_list(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<OsItem> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/os", None, None).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    out.push(OsItem {
                        id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        family: obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        arch: obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        min_ram: obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        is_default: obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                    });
                }
            }
        }
    }
    out
}

pub async fn load_applications(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<ApplicationView> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/applications", None, None).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    out.push(ApplicationView {
                        id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        short_description: obj.get("shortDescription").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        description: obj.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        support_level: obj.get("supportLevel").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        price: obj.get("price").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        tags: obj.get("tags").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    });
                }
            }
        }
    }
    out
}

pub async fn load_instances_for_user(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    users_map: &std::collections::HashMap<String, UserRecord>,
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
