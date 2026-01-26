use super::client::api_call;
use serde::{Deserialize, Serialize};

/// Application (OCA - One Click Application) structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub pricing_type: String,
    pub is_active: bool,
    pub logo_url: Option<String>,
    pub tag: String,
    pub is_experimental: bool,
    pub description: Option<String>,
    pub os_family: String,
    pub os_list: Vec<String>,
    
    // Helper field for backward compatibility with templates
    #[serde(skip)]
    pub category: Option<String>,
}

/// Load available one-click applications from the API
pub async fn load_applications(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<Application> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/applications", None, None).await;
    let mut applications = Vec::new();
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("applications").and_then(|a| a.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        let os_list = if let Some(list) = obj.get("osList").and_then(|v| v.as_array()) {
                            list.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        } else {
                            Vec::new()
                        };
                        
                        let tag = obj.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        // Use tag as category for backward compatibility
                        let category = if !tag.is_empty() {
                            Some(tag.clone())
                        } else {
                            None
                        };
                        
                        applications.push(Application {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            price: obj.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                            pricing_type: obj.get("pricingType").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            is_active: obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(false),
                            logo_url: obj.get("logoUrl").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            tag,
                            is_experimental: obj.get("isExperimental").and_then(|v| v.as_bool()).unwrap_or(false),
                            description: obj.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            os_family: obj.get("osFamily").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            os_list,
                            category,
                        });
                    }
                }
            }
        }
    }
    
    applications
}
