//! What can go wrong talking to the Cloudzy API, and how to say it.

use serde_json::Value;
use thiserror::Error;

use crate::auth::AuthError;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error("could not reach {url}: {source}")]
    Network { url: String, source: reqwest::Error },
    #[error("{}", http_message(*status, code.as_deref(), message))]
    Http {
        status: u16,
        code: Option<String>,
        message: String,
        details: Option<Value>,
    },
    #[error("unexpected response from {url}: {reason}")]
    Decode { url: String, reason: String },
    #[error("{source}\n  {guidance}")]
    Uncertain {
        source: Box<ApiError>,
        guidance: String,
    },
}

impl ApiError {
    /// Build from a non-2xx response body. The platform answers
    /// `{"error": "…", "code": "…", "details": {…}}`; older handlers omit
    /// `code`, and an edge proxy may answer with no JSON at all.
    pub fn from_response(status: u16, body: &[u8]) -> ApiError {
        let parsed: Option<Value> = serde_json::from_slice(body).ok();
        let field = |k: &str| {
            parsed
                .as_ref()
                .and_then(|v| v.get(k))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let mut message = field("error")
            .or_else(|| field("message"))
            .or_else(|| {
                field("title").map(|title| match field("detail") {
                    Some(detail) if !detail.is_empty() => format!("{title}: {detail}"),
                    _ => title,
                })
            })
            .unwrap_or_else(|| {
                let text = String::from_utf8_lossy(body);
                let text = text.trim();
                if parsed.is_some() || text.is_empty() || text.starts_with('<') {
                    reason_phrase(status).to_string()
                } else {
                    text.chars().take(300).collect()
                }
            });
        if let Some(reference) = field("instance").or_else(|| field("requestId")) {
            message.push_str(&format!(" [reference: {reference}]"));
        }
        ApiError::Http {
            status,
            code: field("code").or_else(|| {
                message
                    .contains("at capacity for this plan")
                    .then(|| "capacity_unavailable".into())
            }),
            message,
            details: parsed.as_ref().and_then(|v| {
                if v.get("title").is_some() {
                    Some(v.clone())
                } else {
                    v.get("details").cloned()
                }
            }),
        }
    }

    /// Recognize an older server's unregistered route without hiding an
    /// actual structured not-found error for a plan, region or resource.
    pub fn is_missing_route(&self) -> bool {
        matches!(self, ApiError::Http { status: 404, code: None, message, .. }
            if matches!(message.trim().to_ascii_lowercase().as_str(), "not found" | "404 page not found"))
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Http { status, .. } => Some(*status),
            ApiError::Uncertain { source, .. } => source.status(),
            _ => None,
        }
    }

    /// The error as JSON, for `--output json` and MCP tool results.
    pub fn to_json(&self) -> Value {
        match self {
            ApiError::Uncertain { source, guidance } => {
                let mut value = source.to_json();
                value["recovery"] = serde_json::json!(guidance);
                value
            }
            ApiError::Http {
                status,
                code,
                message,
                details,
            } => serde_json::json!({
                "status": status, "code": code, "error": message, "details": details,
                "hint": hint(*status, code.as_deref()),
            }),
            other => serde_json::json!({ "error": other.to_string() }),
        }
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        400 => "bad request",
        401 => "unauthorized",
        402 => "payment required",
        403 => "forbidden",
        404 => "not found",
        409 => "conflict",
        423 => "locked",
        429 => "too many requests",
        500 => "internal server error",
        502 => "bad gateway",
        503 => "service unavailable",
        504 => "gateway timeout",
        _ => "request failed",
    }
}

/// A next step for the person, where there is an obvious one.
pub fn hint(status: u16, code: Option<&str>) -> Option<&'static str> {
    Some(match (status, code) {
        (_, Some("capacity_unavailable" | "REGION_CAPACITY" | "PLAN_OUT_OF_STOCK")) => "this plan has no live capacity in that region; choose another plan or region. Catalog stock and price quotes do not reserve capacity",
        (401, _) => "the credential was rejected — run `zy login` again, or check CLOUDZY_TOKEN",
        (403, Some("insufficient_scope")) => "the credential lacks the scope this needs — sign in again with `zy login`, or mint a developer token with that scope",
        (403, Some("endpoint_not_permitted")) => "this operation is not available to API clients; use the Cloudzy dashboard",
        (403, Some("email_not_verified")) => "verify your email address in the Cloudzy dashboard first",
        (402, _) => "your balance does not cover this — top up in the Cloudzy dashboard (Billing)",
        (423, _) => "the resource is locked (for example a legal hold)",
        (429, _) => "rate limited — wait a moment and try again",
        _ => return None,
    })
}

fn http_message(status: u16, code: Option<&str>, message: &str) -> String {
    let mut out = format!("{message} (HTTP {status}");
    if let Some(c) = code {
        out.push_str(&format!(", {c}"));
    }
    out.push(')');
    if let Some(h) = hint(status, code) {
        out.push_str(&format!("\n  hint: {h}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_platform_error_shapes() {
        let e = ApiError::from_response(
            403,
            br#"{"error":"api token is missing the required scope: services.delete","code":"insufficient_scope"}"#,
        );
        let text = e.to_string();
        assert!(
            text.contains("services.delete")
                && text.contains("HTTP 403, insufficient_scope")
                && text.contains("hint:")
        );

        let e = ApiError::from_response(400, br#"{"error":"hostname is required"}"#);
        assert_eq!(e.to_string(), "hostname is required (HTTP 400)");

        let e = ApiError::from_response(502, b"<html>Bad Gateway</html>");
        assert_eq!(e.to_string(), "bad gateway (HTTP 502)");

        let e = ApiError::from_response(400, br#"{"error":"validation failed","code":"VALIDATION_ERROR","details":{"field":"planId"}}"#);
        assert_eq!(e.to_json()["details"]["field"], "planId");
    }
}
