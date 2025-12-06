use std::collections::HashMap;

/// Parse URL-encoded form body into a HashMap
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
