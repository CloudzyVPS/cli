use serde_json::{json, Value};

/// Returns the list of MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_instances",
            "description": "List compute instances. Optionally filter by username.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "username": {
                        "type": "string",
                        "description": "Optional username to filter instances by assigned user"
                    }
                }
            }
        }),
        json!({
            "name": "get_instance",
            "description": "Get details of a specific compute instance by its ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "The instance ID to retrieve"
                    }
                },
                "required": ["instance_id"]
            }
        }),
        json!({
            "name": "power_on_instance",
            "description": "Power on a compute instance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "The instance ID to power on"
                    }
                },
                "required": ["instance_id"]
            }
        }),
        json!({
            "name": "power_off_instance",
            "description": "Power off a compute instance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "The instance ID to power off"
                    }
                },
                "required": ["instance_id"]
            }
        }),
        json!({
            "name": "reset_instance",
            "description": "Reset (reboot) a compute instance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "The instance ID to reset"
                    }
                },
                "required": ["instance_id"]
            }
        }),
        json!({
            "name": "delete_instance",
            "description": "Permanently delete a compute instance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "The instance ID to delete"
                    }
                },
                "required": ["instance_id"]
            }
        }),
        json!({
            "name": "list_regions",
            "description": "List available cloud regions.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "list_ssh_keys",
            "description": "List SSH keys associated with the account.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

/// Execute an MCP tool by name with the given arguments.
/// Returns the JSON result to embed in the MCP response.
pub async fn call_tool(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    use crate::api::client::api_call;

    match name {
        "list_instances" => {
            let payload = api_call(client, api_base_url, api_token, "GET", "/v1/instances", None, None).await;
            Ok(payload)
        }
        "get_instance" => {
            let instance_id = arguments.get("instance_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required argument: instance_id")?;
            let endpoint = format!("/v1/instances/{}", instance_id);
            let payload = api_call(client, api_base_url, api_token, "GET", &endpoint, None, None).await;
            Ok(payload)
        }
        "power_on_instance" => {
            let instance_id = arguments.get("instance_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required argument: instance_id")?;
            let body = json!({"instanceId": instance_id});
            let payload = api_call(client, api_base_url, api_token, "POST", "/v1/instances/poweron", Some(body), None).await;
            Ok(payload)
        }
        "power_off_instance" => {
            let instance_id = arguments.get("instance_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required argument: instance_id")?;
            let body = json!({"instanceId": instance_id});
            let payload = api_call(client, api_base_url, api_token, "POST", "/v1/instances/poweroff", Some(body), None).await;
            Ok(payload)
        }
        "reset_instance" => {
            let instance_id = arguments.get("instance_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required argument: instance_id")?;
            let body = json!({"instanceId": instance_id});
            let payload = api_call(client, api_base_url, api_token, "POST", "/v1/instances/reset", Some(body), None).await;
            Ok(payload)
        }
        "delete_instance" => {
            let instance_id = arguments.get("instance_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required argument: instance_id")?;
            let endpoint = format!("/v1/instances/{}", instance_id);
            let payload = api_call(client, api_base_url, api_token, "DELETE", &endpoint, None, None).await;
            Ok(payload)
        }
        "list_regions" => {
            let payload = api_call(client, api_base_url, api_token, "GET", "/v1/regions", None, None).await;
            Ok(payload)
        }
        "list_ssh_keys" => {
            let payload = api_call(client, api_base_url, api_token, "GET", "/v1/ssh-keys", None, None).await;
            Ok(payload)
        }
        _ => Err(format!("unknown tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_returns_expected_tools() {
        let tools = tool_definitions();
        assert!(!tools.is_empty());

        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(names.contains(&"list_instances"));
        assert!(names.contains(&"get_instance"));
        assert!(names.contains(&"power_on_instance"));
        assert!(names.contains(&"power_off_instance"));
        assert!(names.contains(&"reset_instance"));
        assert!(names.contains(&"delete_instance"));
        assert!(names.contains(&"list_regions"));
        assert!(names.contains(&"list_ssh_keys"));
    }

    #[test]
    fn test_tool_definitions_have_input_schemas() {
        let tools = tool_definitions();
        for tool in &tools {
            assert!(tool.get("inputSchema").is_some(), "tool {:?} missing inputSchema", tool.get("name"));
            assert!(tool.get("description").is_some(), "tool {:?} missing description", tool.get("name"));
        }
    }

    #[test]
    fn test_required_fields_on_instance_tools() {
        let tools = tool_definitions();
        for tool in &tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let schema = tool.get("inputSchema").unwrap();
            if name == "get_instance" || name == "power_on_instance" || name == "power_off_instance"
                || name == "reset_instance" || name == "delete_instance"
            {
                let required = schema.get("required").and_then(|r| r.as_array()).expect("missing required field");
                let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
                assert!(required_names.contains(&"instance_id"), "{} should require instance_id", name);
            }
        }
    }
}
