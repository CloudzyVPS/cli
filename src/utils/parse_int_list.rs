/// Parse a list of integers from string values
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
