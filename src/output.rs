//! Rendering API responses for people (tables) and programs (JSON).

use clap::ValueEnum;
use comfy_table::presets::UTF8_BORDERS_ONLY;
use comfy_table::{ContentArrangement, Table};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Format {
    /// Human-readable table
    #[default]
    Table,
    /// The API's JSON, unmodified
    Json,
}

/// A column: header and the JSON pointer (RFC 6901) of its value.
#[derive(Debug, Clone, Copy)]
pub struct Col {
    pub header: &'static str,
    pub pointer: &'static str,
}

pub const fn col(header: &'static str, pointer: &'static str) -> Col {
    Col { header, pointer }
}

/// One cell's text.
pub fn cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "-".to_string(),
        Some(Value::String(s)) if s.is_empty() => "-".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => if *b { "yes" } else { "no" }.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(a)) if a.iter().all(|v| !v.is_object() && !v.is_array()) => {
            if a.is_empty() {
                "-".to_string()
            } else {
                a.iter()
                    .map(|v| cell(Some(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        Some(other) => other.to_string(),
    }
}

fn table() -> Table {
    let mut t = Table::new();
    t.load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);
    t
}

pub fn render_list(rows: &[Value], cols: &[Col]) -> String {
    let mut t = table();
    t.set_header(cols.iter().map(|c| c.header));
    for row in rows {
        t.add_row(cols.iter().map(|c| cell(row.pointer(c.pointer))));
    }
    t.to_string()
}

pub fn render_object(value: &Value, cols: &[Col]) -> String {
    let mut t = table();
    if cols.is_empty() {
        if let Value::Object(map) = value {
            for (k, v) in map {
                t.add_row(vec![k.clone(), cell(Some(v))]);
            }
        } else {
            return cell(Some(value));
        }
    } else {
        for c in cols {
            t.add_row(vec![c.header.to_string(), cell(value.pointer(c.pointer))]);
        }
    }
    t.to_string()
}

pub fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    );
}

/// Print a list response. JSON output is the API's response as received.
pub fn print_list(format: Format, response: &Value, rows: &[Value], cols: &[Col], empty: &str) {
    match format {
        Format::Json => print_json(response),
        Format::Table if rows.is_empty() => eprintln!("{empty}"),
        Format::Table => println!("{}", render_list(rows, cols)),
    }
}

/// Print a single-object response.
pub fn print_object(format: Format, value: &Value, cols: &[Col]) {
    match format {
        Format::Json => print_json(value),
        Format::Table => println!("{}", render_object(value, cols)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cells() {
        assert_eq!(cell(None), "-");
        assert_eq!(cell(Some(&json!(null))), "-");
        assert_eq!(cell(Some(&json!(""))), "-");
        assert_eq!(cell(Some(&json!(true))), "yes");
        assert_eq!(cell(Some(&json!(2048))), "2048");
        assert_eq!(cell(Some(&json!(["a", "b"]))), "a, b");
        assert_eq!(cell(Some(&json!({"k": 1}))), r#"{"k":1}"#);
    }

    #[test]
    fn list_uses_pointers() {
        let rows = vec![json!({"id": "s1", "plan": {"name": "Pro"}})];
        let out = render_list(
            &rows,
            &[
                col("ID", "/id"),
                col("PLAN", "/plan/name"),
                col("IP", "/ipAddress"),
            ],
        );
        assert!(out.contains("s1") && out.contains("Pro") && out.contains("-"));
    }
}
