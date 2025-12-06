use crate::models::ApplicationView;
use super::client::api_call;
use serde_json::Value;

/// Load application catalog from the API.
/// Returns a list of available applications with descriptions and pricing.
pub async fn load_applications(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<ApplicationView> {
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/applications", None, None).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        let candidates = if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            arr.clone()
        } else if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("applications").and_then(|a| a.as_array()) {
                arr.clone()
            } else if let Some(arr) = data.get("apps").and_then(|a| a.as_array()) {
                arr.clone()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        for item in candidates {
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
    out
}
