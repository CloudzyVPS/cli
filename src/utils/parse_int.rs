/// Parse an optional integer from a string
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
