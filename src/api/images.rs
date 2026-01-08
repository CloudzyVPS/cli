use super::client::api_call;
use serde_json::Value;

/// Image view structure
#[derive(Clone, Debug)]
pub struct ImageView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub status: String,
    pub region_id: String,
    pub format: Option<String>,
    #[allow(dead_code)]
    pub decompress: Option<String>,
    #[allow(dead_code)]
    pub created_at: Option<i64>,
}

/// Paginated result structure for images
#[derive(Clone, Debug)]
pub struct PaginatedImages {
    pub images: Vec<ImageView>,
    pub total_count: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

/// Load images from the API
pub async fn load_images(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    page: usize,
    per_page: usize,
) -> PaginatedImages {
    let mut params = Vec::new();
    
    if page >= 1 {
        params.push(("page".to_string(), page.to_string()));
        params.push(("per_page".to_string(), per_page.to_string()));
    }
    
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/images", None, Some(params)).await;
    
    let mut images = Vec::new();
    let mut total_count = 0;
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("images").or_else(|| data.get("data")).and_then(|i| i.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        images.push(ImageView {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            url: obj.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            status: obj.get("status").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string(),
                            region_id: obj.get("regionId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            format: obj.get("format").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            decompress: obj.get("decompress").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            created_at: obj.get("createdAt").and_then(|v| v.as_i64()),
                        });
                    }
                }
            }
            
            total_count = data.get("total").and_then(|t| t.as_u64()).unwrap_or(images.len() as u64) as usize;
        }
    }
    
    let actual_total = if total_count > 0 { total_count } else { images.len() };
    let total_pages = if per_page > 0 { actual_total.div_ceil(per_page) } else { 1 };
    let current_page = if page >= 1 { page } else { 1 };
    
    PaginatedImages {
        images,
        total_count: actual_total,
        current_page,
        total_pages,
        per_page,
    }
}

/// Download and add a custom image
pub async fn download_image(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    name: &str,
    url: &str,
    region_id: &str,
    format: Option<String>,
    decompress: Option<String>,
) -> Value {
    let mut payload = serde_json::json!({
        "name": name,
        "url": url,
        "regionId": region_id
    });
    
    if let Some(fmt) = format {
        payload["format"] = Value::String(fmt);
    }
    
    if let Some(dec) = decompress {
        payload["decompress"] = Value::String(dec);
    }
    
    api_call(client, api_base_url, api_token, "POST", "/v1/images", Some(payload), None).await
}

/// Get image details
pub async fn get_image(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    image_id: &str,
) -> Value {
    let endpoint = format!("/v1/images/{}", image_id);
    api_call(client, api_base_url, api_token, "GET", &endpoint, None, None).await
}

/// Delete an image
pub async fn delete_image(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    image_id: &str,
) -> Value {
    let endpoint = format!("/v1/images/{}", image_id);
    api_call(client, api_base_url, api_token, "DELETE", &endpoint, None, None).await
}
