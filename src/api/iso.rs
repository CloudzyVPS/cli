use super::client::api_call;
use serde_json::Value;

/// ISO view structure
#[derive(Clone, Debug)]
pub struct IsoView {
    // pub id: String,
    pub name: String,
    // pub url: String,
    pub status: String,
    pub region_id: String,
    pub use_virtio: bool,
    // Created timestamp from API - preserved for future sorting/filtering
    // pub created_at: Option<i64>,
}

/// Paginated result structure for ISOs
#[derive(Clone, Debug)]
pub struct PaginatedIsos {
    pub isos: Vec<IsoView>,
    pub total_count: usize,
    // pub current_page: usize,
    // pub total_pages: usize,
    // pub per_page: usize,
}

/// Load ISOs from the API
pub async fn load_isos(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    page: usize,
    per_page: usize,
) -> PaginatedIsos {
    let mut params = Vec::new();
    
    if page >= 1 {
        params.push(("page".to_string(), page.to_string()));
        params.push(("per_page".to_string(), per_page.to_string()));
    }
    
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/iso", None, Some(params)).await;
    
    let mut isos = Vec::new();
    let mut total_count = 0;
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("isos").or_else(|| data.get("data")).and_then(|i| i.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        isos.push(IsoView {
                            // id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            // url: obj.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            status: obj.get("status").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string(),
                            region_id: obj.get("regionId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            use_virtio: obj.get("useVirtio").and_then(|v| v.as_bool()).unwrap_or(true),
                            // created_at: obj.get("createdAt").and_then(|v| v.as_i64()),
                        });
                    }
                }
            }
            
            total_count = data.get("total").and_then(|t| t.as_u64()).unwrap_or(isos.len() as u64) as usize;
        }
    }
    
    let actual_total = if total_count > 0 { total_count } else { isos.len() };
    // let total_pages = if per_page > 0 { actual_total.div_ceil(per_page) } else { 1 };
    // let current_page = if page >= 1 { page } else { 1 };
    
    PaginatedIsos {
        isos,
        total_count: actual_total,
        // current_page,
        // total_pages,
        // per_page,
    }
}

/// Download and add a custom ISO
pub async fn download_iso(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    name: &str,
    url: &str,
    region_id: &str,
    use_virtio: bool,
) -> Value {
    let payload = serde_json::json!({
        "name": name,
        "url": url,
        "regionId": region_id,
        "useVirtio": use_virtio
    });
    api_call(client, api_base_url, api_token, "POST", "/v1/iso", Some(payload), None).await
}

// Get ISO details - preserved for future use
// pub async fn get_iso(
//     client: &reqwest::Client,
//     api_base_url: &str,
//     api_token: &str,
//     iso_id: &str,
// ) -> Value {
//     let endpoint = format!("/v1/iso/{}", iso_id);
//     api_call(client, api_base_url, api_token, "GET", &endpoint, None, None).await
// }

// Delete an ISO - preserved for future use
// pub async fn delete_iso(
//     client: &reqwest::Client,
//     api_base_url: &str,
//     api_token: &str,
//     iso_id: &str,
// ) -> Value {
//     let endpoint = format!("/v1/iso/{}", iso_id);
//     api_call(client, api_base_url, api_token, "DELETE", &endpoint, None, None).await
// }
