use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::workspace_record::{WorkspaceMember, WorkspaceRecord, WorkspaceRole};

const WORKSPACES_FILE: &str = "workspaces.json";

/// Load all workspaces from `workspaces.json`.
/// Returns an empty map if the file does not exist yet.
pub async fn load_workspaces_from_file() -> Arc<Mutex<HashMap<String, WorkspaceRecord>>> {
    let path = std::path::Path::new(WORKSPACES_FILE);
    let mut map: HashMap<String, WorkspaceRecord> = HashMap::new();

    if path.exists() {
        if let Ok(text) = tokio::fs::read_to_string(path).await {
            if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&text) {
                // Support both an array of objects and an object keyed by slug.
                let entries: Vec<serde_json::Value> = if let Some(a) = arr.as_array() {
                    a.clone()
                } else if let Some(obj) = arr.as_object() {
                    obj.values().cloned().collect()
                } else {
                    vec![]
                };

                for entry in entries {
                    if let Some(slug) = entry.get("slug").and_then(|v| v.as_str()) {
                        let name = entry
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(slug)
                            .to_string();
                        let description = entry
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let created_at = entry
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let members = entry
                            .get("members")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|m| {
                                        let username = m
                                            .get("username")
                                            .and_then(|v| v.as_str())?
                                            .to_string();
                                        let role_str = m
                                            .get("role")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("viewer");
                                        let role = WorkspaceRole::from_str(role_str)
                                            .unwrap_or(WorkspaceRole::Viewer);
                                        Some(WorkspaceMember { username, role })
                                    })
                                    .collect()
                            })
                            .unwrap_or_else(Vec::new);

                        map.insert(
                            slug.to_string(),
                            WorkspaceRecord {
                                name,
                                description,
                                slug: slug.to_string(),
                                created_at,
                                members,
                            },
                        );
                    }
                }
            }
        }
    }

    Arc::new(Mutex::new(map))
}

/// Persist the current workspace map to `workspaces.json`.
pub async fn persist_workspaces_file(
    workspaces_arc: &Arc<Mutex<HashMap<String, WorkspaceRecord>>>,
) -> Result<(), std::io::Error> {
    let content = {
        let workspaces = workspaces_arc.lock().unwrap();
        let arr: Vec<serde_json::Value> = workspaces
            .values()
            .map(|ws| {
                let members: Vec<serde_json::Value> = ws
                    .members
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "username": m.username,
                            "role": m.role.as_str()
                        })
                    })
                    .collect();
                serde_json::json!({
                    "slug": ws.slug,
                    "name": ws.name,
                    "description": ws.description,
                    "created_at": ws.created_at,
                    "members": members
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::Value::Array(arr))?
    };
    tokio::fs::write(WORKSPACES_FILE, content).await
}

/// Generate a URL-safe slug from a display name.
/// Converts to lowercase, replaces spaces and special chars with `-`,
/// and trims leading/trailing dashes.
pub fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect();
    // Collapse consecutive dashes and trim edges.
    let collapsed = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    collapsed
}

/// Returns the current UTC timestamp as an ISO-8601 string.
pub fn now_iso8601() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Production Team"), "production-team");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("My Workspace!!! 2024"), "my-workspace-2024");
    }

    #[test]
    fn slugify_already_slug() {
        assert_eq!(slugify("my-workspace"), "my-workspace");
    }

    #[test]
    fn slugify_consecutive_special() {
        assert_eq!(slugify("hello   world"), "hello-world");
    }
}
