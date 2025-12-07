use serde_json::Value;
use yansi::Paint;
use std::sync::atomic::{AtomicBool, Ordering};

static SILENT: AtomicBool = AtomicBool::new(false);

pub fn set_silent(silent: bool) {
    SILENT.store(silent, Ordering::Relaxed);
}

fn log_output(msg: String) {
    if !SILENT.load(Ordering::Relaxed) {
        println!("{}", msg);
    }
}

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
    // --- Curl Logging ---
    let mut url_for_log = format!("{}{}", api_base_url, endpoint);
    if let Some(ref p) = params {
        if !p.is_empty() {
             let query_string = p.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
             url_for_log = format!("{}?{}", url_for_log, query_string);
        }
    }

    let mut parts = Vec::new();
    parts.push(Paint::new("curl").fg(yansi::Color::Green).bold().to_string());
    parts.push(format!("-X {}", Paint::new(method).fg(yansi::Color::Yellow).bold()));
    parts.push(format!("'{}'", Paint::new(&url_for_log).fg(yansi::Color::Cyan)));

    if !api_token.is_empty() {
        parts.push(format!("{} {}", 
            Paint::new("-H").fg(yansi::Color::Magenta), 
            Paint::new(format!("'API-Token: {}'", api_token)).fg(yansi::Color::Magenta)
        ));
    }
    if body.is_some() {
        parts.push(format!("{} {}", 
            Paint::new("-H").fg(yansi::Color::Magenta), 
            Paint::new("'Content-Type: application/json'").fg(yansi::Color::Magenta)
        ));
    }

    if let Some(ref d) = body {
        let json_str = serde_json::to_string_pretty(d).unwrap_or_default();
        let escaped_json = json_str.replace("'", "'\\''");
        parts.push(format!("{} {}", 
            Paint::new("-d").fg(yansi::Color::Blue), 
            Paint::new(format!("'{}'", escaped_json)).fg(yansi::Color::White)
        ));
    }
    log_output(format!("Request:\n{}", parts.join(" ")));
    // --------------------

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
    
    if let Some(ref p) = params {
        req = req.query(p);
    }
    
    if let Some(ref b) = body {
        req = req.json(b);
    }
    
    let result = match req.send().await {
        Ok(resp) => resp.json().await.unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse response"})),
        Err(e) => serde_json::json!({"error": format!("Request failed: {}", e)}),
    };

    // Colorize the response JSON for better readability in the terminal
    let json_str = serde_json::to_string(&result).unwrap_or_else(|_| format!("{:?}", result));
    // Grayed out color (dimmed/dark gray)
    let response_str = Paint::new(json_str).rgb(100, 100, 100).to_string();
    log_output(format!("Response:\n{}", response_str));

    result
}
