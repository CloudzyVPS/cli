use serde::{Deserialize, Serialize};

/// Role a user holds within a specific workspace.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    /// Full control over workspace resources and membership.
    Manager,
    /// Can create and modify resources inside the workspace.
    Editor,
    /// Read-only access to workspace resources.
    Viewer,
}

impl WorkspaceRole {
    /// Human-readable label shown in the UI.
    pub fn label(&self) -> &'static str {
        match self {
            WorkspaceRole::Manager => "Manager",
            WorkspaceRole::Editor => "Editor",
            WorkspaceRole::Viewer => "Viewer",
        }
    }

    /// Short description of what this workspace role can do.
    pub fn description(&self) -> &'static str {
        match self {
            WorkspaceRole::Manager => {
                "Full access: manage members, create/delete resources and change workspace settings."
            }
            WorkspaceRole::Editor => {
                "Can create, modify and delete resources within the workspace but cannot manage members."
            }
            WorkspaceRole::Viewer => {
                "Read-only access. Can view resources but cannot make any changes."
            }
        }
    }

    /// Parse from the string value stored in JSON.
    pub fn from_str(s: &str) -> Option<WorkspaceRole> {
        match s {
            "manager" => Some(WorkspaceRole::Manager),
            "editor" => Some(WorkspaceRole::Editor),
            "viewer" => Some(WorkspaceRole::Viewer),
            _ => None,
        }
    }

    /// Serialise to the string value stored in JSON.
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkspaceRole::Manager => "manager",
            WorkspaceRole::Editor => "editor",
            WorkspaceRole::Viewer => "viewer",
        }
    }

    /// All valid workspace roles, in display order.
    #[allow(dead_code)]
    pub fn all() -> &'static [WorkspaceRole] {
        &[WorkspaceRole::Manager, WorkspaceRole::Editor, WorkspaceRole::Viewer]
    }
}

/// A single user's membership inside a workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub username: String,
    pub role: WorkspaceRole,
}

/// A workspace groups resources and members under a shared context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    /// Human-readable display name.
    pub name: String,
    /// Optional free-text description.
    #[serde(default)]
    pub description: String,
    /// URL-safe slug (also used as the map key).
    pub slug: String,
    /// ISO-8601 creation timestamp.
    #[serde(default)]
    pub created_at: String,
    /// Members of this workspace and their workspace-level roles.
    #[serde(default)]
    pub members: Vec<WorkspaceMember>,
    /// Instance IDs that belong to this workspace.
    #[serde(default)]
    pub assigned_instances: Vec<String>,
}

impl WorkspaceRecord {
    /// Returns true if the given instance ID is assigned to this workspace.
    pub fn has_instance(&self, id: &str) -> bool {
        self.assigned_instances.iter().any(|i| i == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_role_roundtrip() {
        for role in WorkspaceRole::all() {
            let s = role.as_str();
            let parsed = WorkspaceRole::from_str(s).expect("should parse back");
            assert_eq!(role, &parsed);
        }
    }

    #[test]
    fn workspace_role_invalid_returns_none() {
        assert!(WorkspaceRole::from_str("superuser").is_none());
    }

    #[test]
    fn workspace_role_labels_non_empty() {
        for role in WorkspaceRole::all() {
            assert!(!role.label().is_empty());
            assert!(!role.description().is_empty());
        }
    }
}
