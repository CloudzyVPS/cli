use serde::{Deserialize, Serialize};

/// Valid global system roles. 
///
/// - `owner`  – full administrative access, can manage users and workspaces.
/// - `admin`  – can manage resources (instances, snapshots, …) for their assigned
///              instances / workspaces, but cannot manage users or global settings.
/// - `viewer` – read-only access to all resources they are assigned to.
#[derive(Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub password: String,
    /// One of `"owner"`, `"admin"`, or `"viewer"`.
    pub role: String,
    pub assigned_instances: Vec<String>,
    #[serde(default)]
    pub about: String,
}

impl UserRecord {
    /// Returns `true` for any recognised role value.
    pub fn is_valid_role(role: &str) -> bool {
        matches!(role, "owner" | "admin" | "viewer")
    }

    /// Human-readable label for the role.
    #[allow(dead_code)]
    pub fn role_label(role: &str) -> &'static str {
        match role {
            "owner" => "Owner",
            "admin" => "Admin",
            "viewer" => "Viewer",
            _ => "Unknown",
        }
    }

    /// Short description of what the role can do.
    #[allow(dead_code)]
    pub fn role_description(role: &str) -> &'static str {
        match role {
            "owner" => "Full system access: manage users, workspaces, and all resources.",
            "admin" => "Manage assigned resources (instances, snapshots, etc.) and workspaces.",
            "viewer" => "Read-only access to assigned resources. Cannot make any changes.",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_roles_accepted() {
        assert!(UserRecord::is_valid_role("owner"));
        assert!(UserRecord::is_valid_role("admin"));
        assert!(UserRecord::is_valid_role("viewer"));
    }

    #[test]
    fn invalid_roles_rejected() {
        assert!(!UserRecord::is_valid_role("superadmin"));
        assert!(!UserRecord::is_valid_role(""));
        assert!(!UserRecord::is_valid_role("OWNER"));
    }
}
