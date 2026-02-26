use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use axum::Json;
use serde::Deserialize;

use crate::mcp::tools;
use crate::models::AppState;

/// GET /mcp/tools — returns tool definitions as JSON (the raw MCP self-description).
pub async fn mcp_tools_json() -> impl IntoResponse {
    Json(tools::tool_definitions())
}

/// GET /mcp — serves the interactive MCP documentation page.
/// This is a self-contained HTML page with embedded CSS and JS that fetches
/// tool definitions from /mcp/tools and renders them in a Swagger-like UI.
pub async fn mcp_docs_page() -> impl IntoResponse {
    Html(include_str!("mcp_docs.html"))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

/// GET /mcp/logs — returns paginated MCP call logs as JSON.
pub async fn mcp_logs_json(
    State(state): State<AppState>,
    Query(q): Query<LogsQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(1);
    let per_page = q.per_page.unwrap_or(20);
    Json(state.mcp_log_store.list(page, per_page))
}

/// GET /mcp/logs/:id — returns a single log entry as JSON.
pub async fn mcp_log_detail_json(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.mcp_log_store.get(id) {
        Some(entry) => Json(serde_json::to_value(entry).expect("McpLogEntry serialization")).into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not found"}))).into_response(),
    }
}

/// GET /mcp/logs-page — serves the MCP call logs viewer HTML page.
pub async fn mcp_logs_page() -> impl IntoResponse {
    Html(include_str!("mcp_logs.html"))
}
