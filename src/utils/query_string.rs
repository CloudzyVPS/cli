use urlencoding::encode;

/// Build a query string from key-value pairs
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
