use std::collections::HashMap;
use crate::models::Region;
use super::client::api_call;

/// Load all available regions from the API.
/// Returns a vector of regions and a hashmap for quick lookup by ID.
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
