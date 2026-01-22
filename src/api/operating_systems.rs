use crate::models::OsItem;
use super::client::api_call;

/// Load operating system catalog from the API.
/// Returns a list of available OS images with their details.
pub async fn load_os_list(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Vec<OsItem> {
    let params = vec![("per_page".to_string(), "1000".to_string())];
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/os", None, Some(params)).await;
    let mut out = vec![];
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("os").and_then(|o| o.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        out.push(OsItem {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            family: obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            arch: obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            min_ram: obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            is_default: obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                            is_active: obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true),
                        });
                    }
                }
            }
        }
    }
    out
}
