use super::client::api_call;
use serde_json::Value;

/// Floating IP view structure
#[derive(Clone, Debug)]
pub struct FloatingIpView {
    pub id: String,
    pub ip_address: String,
    pub region_id: String,
    pub instance_id: Option<String>,
    pub auto_renew: bool,
    pub customer_note: Option<String>,
    #[allow(dead_code)]
    pub created_at: Option<i64>,
}

/// Paginated result structure for floating IPs
#[derive(Clone, Debug)]
pub struct PaginatedFloatingIps {
    pub floating_ips: Vec<FloatingIpView>,
    pub total_count: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

/// Load floating IPs from the API
pub async fn load_floating_ips(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    page: usize,
    per_page: usize,
) -> PaginatedFloatingIps {
    let mut params = Vec::new();
    
    if page >= 1 {
        params.push(("page".to_string(), page.to_string()));
        params.push(("per_page".to_string(), per_page.to_string()));
    }
    
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/floating-ips", None, Some(params)).await;
    
    let mut floating_ips = Vec::new();
    let mut total_count = 0;
    
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("floatingIps").and_then(|f| f.as_array()) {
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        floating_ips.push(FloatingIpView {
                            id: obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            ip_address: obj.get("ipAddress").or_else(|| obj.get("ip")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            region_id: obj.get("regionId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            instance_id: obj.get("instanceId").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            auto_renew: obj.get("autoRenew").and_then(|v| v.as_bool()).unwrap_or(false),
                            customer_note: obj.get("customerNote").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            created_at: obj.get("createdAt").and_then(|v| v.as_i64()),
                        });
                    }
                }
            }
            
            total_count = data.get("total").and_then(|t| t.as_u64()).unwrap_or(floating_ips.len() as u64) as usize;
        }
    }
    
    let actual_total = if total_count > 0 { total_count } else { floating_ips.len() };
    let total_pages = if per_page > 0 { actual_total.div_ceil(per_page) } else { 1 };
    let current_page = if page >= 1 { page } else { 1 };
    
    PaginatedFloatingIps {
        floating_ips,
        total_count: actual_total,
        current_page,
        total_pages,
        per_page,
    }
}

/// Create floating IPs
pub async fn create_floating_ips(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    region_id: &str,
    count: i32,
) -> Value {
    let payload = serde_json::json!({
        "regionId": region_id,
        "count": count
    });
    api_call(client, api_base_url, api_token, "POST", "/v1/floating-ips", Some(payload), None).await
}

/// Update floating IP
pub async fn update_floating_ip(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    ip_id: &str,
    auto_renew: Option<bool>,
    customer_note: Option<String>,
) -> Value {
    let mut payload = serde_json::Map::new();
    
    if let Some(ar) = auto_renew {
        payload.insert("autoRenew".to_string(), Value::Bool(ar));
    }
    
    if let Some(note) = customer_note {
        payload.insert("customerNote".to_string(), Value::String(note));
    }
    
    let endpoint = format!("/v1/floating-ips/{}", ip_id);
    api_call(client, api_base_url, api_token, "PATCH", &endpoint, Some(Value::Object(payload)), None).await
}

/// Release floating IP
pub async fn release_floating_ip(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    ip_id: &str,
) -> Value {
    let endpoint = format!("/v1/floating-ips/{}/release", ip_id);
    api_call(client, api_base_url, api_token, "POST", &endpoint, None, None).await
}
