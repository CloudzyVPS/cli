use std::collections::HashMap;
use urlencoding::encode;

pub fn parse_urlencoded_body(body: &axum::body::Bytes) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let raw = String::from_utf8_lossy(body);
    for pair in raw.split('&') {
        if pair.is_empty() { continue; }
        let mut parts = pair.splitn(2, '=');
        let key_enc = parts.next().unwrap_or("");
        let val_enc = parts.next().unwrap_or("");
        let key = urlencoding::decode(key_enc).unwrap_or_else(|_| key_enc.into()).to_string();
        let val = urlencoding::decode(val_enc).unwrap_or_else(|_| val_enc.into()).to_string();
        map.entry(key).or_default().push(val);
    }
    map
}

pub fn hostname_from_url(u: &str) -> String {
    let s = u.trim();
    if s.is_empty() {
        return "".into();
    }
    let s = if let Some(idx) = s.find("://") { &s[idx+3..] } else { s };
    let host = s.split('/').next().unwrap_or(s);
    host.to_string()
}

pub fn absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let mut base = base_url.to_string();
    if !path.starts_with('/') {
        base.push('/');
        base.push_str(path);
        return base;
    }
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return base;
    }
    format!("{}/{}", base, trimmed)
}

pub fn build_query_string(pairs: &[(String, String)]) -> String {
    let mut first = true;
    let mut out = String::new();
    for (k, v) in pairs {
        if !first {
            out.push('&');
        } else {
            first = false;
        }
        out.push_str(&encode(k));
        out.push('=');
        out.push_str(&encode(v));
    }
    out
}

pub fn parse_flag(value: Option<&String>, default: bool) -> bool {
    match value {
        Some(v) => {
            let t = v.trim().to_lowercase();
            if t.is_empty() {
                default
            } else {
                matches!(t.as_str(), "1" | "true" | "yes" | "on")
            }
        }
        None => default,
    }
}

pub fn parse_optional_int(value: Option<&String>) -> Option<i32> {
    value.and_then(|v| {
        let t = v.trim();
        if t.is_empty() {
            None
        } else {
            t.parse::<i32>().ok()
        }
    })
}

pub fn parse_int_list(values: &[String]) -> Vec<i64> {
    values
        .iter()
        .filter_map(|v| {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<i64>().ok()
            }
        })
        .collect()
}

pub fn value_to_short_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(value_to_short_string)
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(obj) => {
            let mut parts = Vec::new();
            for (key, val) in obj {
                parts.push(format!("{}: {}", key, value_to_short_string(val)));
            }
            parts.join(", ")
        }
        serde_json::Value::Null => String::new(),
    }
}
