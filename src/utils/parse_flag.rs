/// Parse a boolean flag from an optional string value
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
