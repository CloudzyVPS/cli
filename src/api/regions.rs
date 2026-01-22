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
    let params = vec![("per_page".to_string(), "1000".to_string())];
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/regions", None, Some(params)).await;
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
                    
                    // New OpenAPI-aligned fields
                    let abbr = obj.get("abbr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let image = obj.get("image").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let is_active = obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true);
                    let is_out_of_stock = obj.get("isOutOfStock").and_then(|v| v.as_bool()).unwrap_or(false);
                    let overall_activeness = obj.get("overallActiveness").and_then(|v| v.as_bool());
                    let ddos_activeness = obj.get("ddosActiveness").and_then(|v| v.as_bool());
                    let is_premium = obj.get("isPremium").and_then(|v| v.as_bool()).unwrap_or(false);
                    let is_hidden = obj.get("isHidden").and_then(|v| v.as_bool()).unwrap_or(false);
                    let has_offset_price = obj.get("hasOffsetPrice").and_then(|v| v.as_bool()).unwrap_or(false);
                    let max_discount_percent = obj.get("maxDiscountPercent").and_then(|v| v.as_f64());
                    let position = obj.get("position").and_then(|v| v.as_i64()).map(|n| n as i32);
                    let config = obj.get("config").cloned();

                    let region = Region {
                        id: id.clone(),
                        name,
                        slug,
                        country,
                        city,
                        latitude,
                        longitude,
                        abbr,
                        image,
                        is_active,
                        is_out_of_stock,
                        overall_activeness,
                        ddos_activeness,
                        is_premium,
                        is_hidden,
                        has_offset_price,
                        max_discount_percent,
                        position,
                        config,
                    };
                    regions.push(region.clone());
                    map.insert(id, region);
                }
            }
        }
    }
    (regions, map)
}
