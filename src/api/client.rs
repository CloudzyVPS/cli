use serde_json::Value;

/// Core HTTP client function for making API calls.
/// Handles authentication, request building, and error responses.
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
        req = req.header("API-Token", api_token);
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
