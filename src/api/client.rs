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
        _ => {
            tracing::warn!("Unsupported HTTP method: {}, defaulting to GET", method);
            client.get(&url)
        },
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
        Ok(resp) => {
            let status = resp.status();
            match resp.json().await {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!("Failed to parse API response: {}", e);
                    serde_json::json!({
                        "error": "Failed to parse response",
                        "status": status.as_u16()
                    })
                }
            }
        },
        Err(e) => {
            tracing::error!("API request failed: {}", e);
            serde_json::json!({
                "error": format!("Request failed: {}", e)
            })
        }
    }
}
