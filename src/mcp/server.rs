use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::tools;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP server, reading JSON-RPC messages from stdin and writing
/// responses to stdout. Logging goes to stderr so it never contaminates the
/// protocol stream.
pub async fn run(client: reqwest::Client, api_base_url: String, api_token: String) {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                let _ = write_message(&mut stdout, &err_response).await;
                continue;
            }
        };

        // Notifications (no "id" field) are acknowledged silently
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        if id.is_none() {
            // This is a notification (e.g. notifications/initialized) – no response needed
            continue;
        }

        let response = match method {
            "initialize" => handle_initialize(&id, &params),
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &params, &client, &api_base_url, &api_token).await,
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            }),
        };

        if write_message(&mut stdout, &response).await.is_err() {
            break;
        }
    }
}

async fn write_message(stdout: &mut tokio::io::Stdout, msg: &Value) -> Result<(), std::io::Error> {
    let serialized = serde_json::to_string(msg).unwrap_or_default();
    stdout.write_all(serialized.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

fn handle_initialize(id: &Option<Value>, _params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "zy",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list(id: &Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": tools::tool_definitions()
        }
    })
}

async fn handle_tools_call(
    id: &Option<Value>,
    params: &Value,
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tools::call_tool(client, api_base_url, api_token, tool_name, &arguments).await {
        Ok(result) => {
            let text = serde_json::to_string_pretty(&result).unwrap_or_default();
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }
            })
        }
        Err(e) => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": e
                    }],
                    "isError": true
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_initialize() {
        let id = Some(json!(1));
        let params = json!({
            "protocolVersion": "2024-11-05",
            "clientInfo": { "name": "test", "version": "0.1" }
        });
        let resp = handle_initialize(&id, &params);

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "zy");
    }

    #[test]
    fn test_handle_tools_list() {
        let id = Some(json!(2));
        let resp = handle_tools_list(&id);

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 2);
        let tools = resp["result"]["tools"].as_array().expect("tools should be array");
        assert!(!tools.is_empty());
    }

    #[test]
    fn test_handle_initialize_includes_version() {
        let id = Some(json!("init-1"));
        let resp = handle_initialize(&id, &json!({}));
        let version = resp["result"]["serverInfo"]["version"].as_str().unwrap();
        assert!(!version.is_empty());
    }
}
