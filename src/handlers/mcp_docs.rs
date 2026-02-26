use axum::response::{Html, IntoResponse};
use axum::Json;

use crate::mcp::tools;

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
