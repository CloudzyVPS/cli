/// Extract hostname from a URL string
pub fn hostname_from_url(u: &str) -> String {
    let s = u.trim();
    if s.is_empty() {
        return "".into();
    }
    let s = if let Some(idx) = s.find("://") { &s[idx+3..] } else { s };
    let host = s.split('/').next().unwrap_or(s);
    host.to_string()
}
