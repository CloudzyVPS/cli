//! MCP over stdio: newline-delimited JSON-RPC 2.0.
//!
//! stdout carries protocol messages only; anything for a human goes to
//! stderr.

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::tools;
use crate::api::ApiClient;

/// Newest first. A client asking for one of these gets it back; anything else
/// gets the newest, per the spec's version negotiation.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Tools for the Cloudzy cloud: servers, snapshots, SSH keys, IPs, firewall rules, catalog and billing. \
Creating servers, snapshots and IPs charges the account balance. Tools marked destructive delete data, replace credentials \
or release addresses — confirm with the person before calling them. IDs come from the list_* tools.";

/// The API client, or why there is none (for example: not signed in). The
/// server still starts without one, so the client can show the reason on
/// every call instead of a dead process.
pub type Backend = Result<ApiClient, String>;

pub struct Server {
    backend: Backend,
}

impl Server {
    pub fn new(backend: Backend) -> Server {
        Server { backend }
    }

    /// Handle one message; `None` for notifications, which get no reply.
    pub async fn handle(&self, raw: &str) -> Option<Value> {
        let msg: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        let id = msg.get("id").cloned();
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // A response to something we never send, or garbage.
            return id.map(|id| error(id, -32600, "invalid request"));
        };
        let id = id?;
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => {
                let asked = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = PROTOCOL_VERSIONS
                    .iter()
                    .find(|v| **v == asked)
                    .unwrap_or(&PROTOCOL_VERSIONS[0]);
                result(
                    id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": { "tools": { "listChanged": false } },
                        "serverInfo": { "name": "cloudzy", "title": "Cloudzy", "version": env!("CARGO_PKG_VERSION") },
                        "instructions": INSTRUCTIONS,
                    }),
                )
            }
            "ping" => result(id, json!({})),
            "tools/list" => result(
                id,
                json!({ "tools": tools::all().iter().map(tools::Tool::to_json).collect::<Vec<_>>() }),
            ),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(id, -32602, "tools/call needs a tool name"));
                };
                if !tools::all().iter().any(|t| t.name == name) {
                    return Some(error(id, -32602, &format!("unknown tool: {name}")));
                }
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                result(id, self.call_tool(name, &args).await)
            }
            other => error(id, -32601, &format!("method not found: {other}")),
        })
    }

    async fn call_tool(&self, name: &str, args: &Value) -> Value {
        let api = match &self.backend {
            Ok(api) => api,
            Err(why) => return tool_error(why, None),
        };
        match tools::call(api, name, args).await {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                let structured = match value {
                    Value::Object(_) => value,
                    Value::Null => json!({ "ok": true }),
                    other => json!({ "items": other }),
                };
                json!({ "content": [{ "type": "text", "text": text }], "structuredContent": structured, "isError": false })
            }
            Err(e) => tool_error(&e.message, e.detail),
        }
    }
}

fn tool_error(message: &str, detail: Option<Value>) -> Value {
    let mut out = json!({ "content": [{ "type": "text", "text": message }], "isError": true });
    if let Some(d) = detail {
        out["structuredContent"] = d;
    }
    out
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Serve until stdin closes.
pub async fn run(server: Server) -> std::io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle(&line).await {
            let mut bytes = serde_json::to_vec(&reply).expect("reply serializes");
            bytes.push(b'\n');
            stdout.write_all(&bytes).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offline() -> Server {
        Server::new(Err("not signed in — run `zy login`".into()))
    }

    #[tokio::test]
    async fn initialize_negotiates_the_version() {
        let s = offline();
        let r = s.handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#).await.unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "cloudzy");
        let r = s.handle(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).await.unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[tokio::test]
    async fn notifications_get_no_reply_and_errors_are_json_rpc() {
        let s = offline();
        assert!(s
            .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await
            .is_none());
        assert_eq!(
            s.handle("{not json").await.unwrap()["error"]["code"],
            -32700
        );
        assert_eq!(
            s.handle(r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#)
                .await
                .unwrap()["error"]["code"],
            -32601
        );
        assert_eq!(
            s.handle(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope"}}"#)
                .await
                .unwrap()["error"]["code"],
            -32602
        );
    }

    #[tokio::test]
    async fn without_a_credential_tools_fail_visibly() {
        let r = offline().handle(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_servers"}}"#).await.unwrap();
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("zy login"));
    }

    #[tokio::test]
    async fn tools_list_carries_annotations() {
        let r = offline()
            .handle(r#"{"jsonrpc":"2.0","id":6,"method":"tools/list"}"#)
            .await
            .unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        assert!(tools.len() > 30);
        let del = tools.iter().find(|t| t["name"] == "delete_server").unwrap();
        assert_eq!(del["annotations"]["destructiveHint"], true);
        assert_eq!(del["inputSchema"]["required"][0], "id");
    }
}
