/// Build an absolute URL from a base URL and a path
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
