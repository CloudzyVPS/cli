use super::client::api_call;

/// Application (OCA - One Click Application) structure
#[derive(Clone, Debug)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Logo URL from API - preserved for future UI enhancements
    pub logo_url: Option<String>,
    pub category: Option<String>,
    /// OS compatibility list from API - preserved for filtering features
    pub os_compatibility: Vec<String>,
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
                        let os_compat = if let Some(compat) = obj.get("osCompatibility").and_then(|v| v.as_array()) {
                            compat.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        } else {
                            Vec::new()
                        };
                        
                        applications.push(Application {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            description: obj.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            logo_url: obj.get("logoUrl").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            category: obj.get("category").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            os_compatibility: os_compat,
                        });
                    }
                }
            }
        }
    }
    
    applications
}
