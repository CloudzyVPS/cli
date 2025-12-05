/// Convert a JSON value to a short string representation
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
