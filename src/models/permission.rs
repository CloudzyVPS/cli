use serde::{Deserialize, Serialize};

/// Fine-grained action a user may perform.
/// Every route/operation is mapped to one of these actions, allowing
/// the authorisation layer to decide based on role + workspace membership.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // ── Instance actions ───────────────────────────────────────────────
    /// View the list of instances and their basic details.
    ViewInstances,
    /// Create a brand-new instance through the wizard.
    CreateInstance,
    /// Permanently delete an instance.
    DeleteInstance,
    /// Power on a stopped instance.
    PowerOnInstance,
    /// Gracefully power off a running instance.
    PowerOffInstance,
    /// Hard-reset (force-reboot) an instance.
    ResetInstance,
    /// Change the root/admin password of an instance.
    ChangeInstancePassword,
    /// Rebuild an instance with a different operating system.
    RebuildInstance,
    /// Resize (upgrade/downgrade) an instance's plan.
    ResizeInstance,
    /// Purchase and apply additional traffic to an instance.
    AddTrafficToInstance,

    // ── Snapshot actions ───────────────────────────────────────────────
    /// View the snapshot list.
    ViewSnapshots,
    /// Create a snapshot from an instance.
    CreateSnapshot,
    /// Restore an instance from a snapshot.
    RestoreSnapshot,
    /// Delete a snapshot.
    DeleteSnapshot,

    // ── Floating IP actions ────────────────────────────────────────────
    /// View floating IPs.
    ViewFloatingIps,
    /// Allocate a new floating IP.
    CreateFloatingIp,
    /// Reassign a floating IP to a different instance.
    UpdateFloatingIp,
    /// Release (delete) a floating IP.
    ReleaseFloatingIp,

    // ── SSH key actions ────────────────────────────────────────────────
    /// View SSH keys.
    ViewSshKeys,
    /// Upload a new SSH key.
    CreateSshKey,

    // ── ISO / Image / Backup actions ───────────────────────────────────
    /// View custom ISOs.
    ViewIsos,
    /// Download / import a custom ISO.
    ImportIso,
    /// View OS images.
    ViewImages,
    /// Download / import an OS image.
    ImportImage,
    /// View backups.
    ViewBackups,
    /// Create a backup.
    CreateBackup,

    // ── Workspace actions ──────────────────────────────────────────────
    /// View the workspace list and their details.
    ViewWorkspaces,
    /// Create a new workspace.
    CreateWorkspace,
    /// Edit workspace metadata (name, description).
    EditWorkspace,
    /// Add or remove members from a workspace.
    ManageWorkspaceMembers,
    /// Delete a workspace entirely.
    DeleteWorkspace,

    // ── User management actions (owner-only by default) ────────────────
    /// View the user list.
    ViewUsers,
    /// Create a new system user.
    CreateUser,
    /// Update a user's role.
    UpdateUserRole,
    /// Reset another user's password.
    ResetUserPassword,
    /// Update the "about" field on a user profile.
    UpdateUserAbout,
    /// Delete a user account.
    DeleteUser,
    /// View and modify admin ↔ instance assignments.
    ManageAccessAssignments,
}

impl Permission {
    /// Human-readable name shown in the UI.
    pub fn label(&self) -> &'static str {
        match self {
            Permission::ViewInstances => "View Instances",
            Permission::CreateInstance => "Create Instance",
            Permission::DeleteInstance => "Delete Instance",
            Permission::PowerOnInstance => "Power On Instance",
            Permission::PowerOffInstance => "Power Off Instance",
            Permission::ResetInstance => "Reset Instance",
            Permission::ChangeInstancePassword => "Change Instance Password",
            Permission::RebuildInstance => "Rebuild Instance",
            Permission::ResizeInstance => "Resize Instance",
            Permission::AddTrafficToInstance => "Add Traffic to Instance",
            Permission::ViewSnapshots => "View Snapshots",
            Permission::CreateSnapshot => "Create Snapshot",
            Permission::RestoreSnapshot => "Restore Snapshot",
            Permission::DeleteSnapshot => "Delete Snapshot",
            Permission::ViewFloatingIps => "View Floating IPs",
            Permission::CreateFloatingIp => "Create Floating IP",
            Permission::UpdateFloatingIp => "Update Floating IP",
            Permission::ReleaseFloatingIp => "Release Floating IP",
            Permission::ViewSshKeys => "View SSH Keys",
            Permission::CreateSshKey => "Upload SSH Key",
            Permission::ViewIsos => "View Custom ISOs",
            Permission::ImportIso => "Import Custom ISO",
            Permission::ViewImages => "View Images",
            Permission::ImportImage => "Import Image",
            Permission::ViewBackups => "View Backups",
            Permission::CreateBackup => "Create Backup",
            Permission::ViewWorkspaces => "View Workspaces",
            Permission::CreateWorkspace => "Create Workspace",
            Permission::EditWorkspace => "Edit Workspace",
            Permission::ManageWorkspaceMembers => "Manage Workspace Members",
            Permission::DeleteWorkspace => "Delete Workspace",
            Permission::ViewUsers => "View Users",
            Permission::CreateUser => "Create User",
            Permission::UpdateUserRole => "Update User Role",
            Permission::ResetUserPassword => "Reset User Password",
            Permission::UpdateUserAbout => "Update User About",
            Permission::DeleteUser => "Delete User",
            Permission::ManageAccessAssignments => "Manage Access Assignments",
        }
    }

    /// Short description explaining what this permission controls.
    pub fn description(&self) -> &'static str {
        match self {
            Permission::ViewInstances => "See the list of instances and their basic details.",
            Permission::CreateInstance => "Launch a new instance via the creation wizard.",
            Permission::DeleteInstance => "Permanently destroy an instance and free its resources.",
            Permission::PowerOnInstance => "Start a powered-off instance.",
            Permission::PowerOffInstance => "Gracefully shut down a running instance.",
            Permission::ResetInstance => "Force-reboot an unresponsive instance.",
            Permission::ChangeInstancePassword => "Change the root or admin password for an instance.",
            Permission::RebuildInstance => "Reinstall an instance with a different OS image.",
            Permission::ResizeInstance => "Upgrade or downgrade an instance to a different plan.",
            Permission::AddTrafficToInstance => "Purchase extra bandwidth for an instance.",
            Permission::ViewSnapshots => "Browse the snapshot list.",
            Permission::CreateSnapshot => "Take a point-in-time snapshot of an instance.",
            Permission::RestoreSnapshot => "Restore an instance to a previous snapshot state.",
            Permission::DeleteSnapshot => "Remove a snapshot and free storage.",
            Permission::ViewFloatingIps => "Browse allocated floating IP addresses.",
            Permission::CreateFloatingIp => "Allocate a new floating IP address.",
            Permission::UpdateFloatingIp => "Reassign a floating IP to a different instance.",
            Permission::ReleaseFloatingIp => "Release a floating IP and stop billing.",
            Permission::ViewSshKeys => "Browse uploaded SSH public keys.",
            Permission::CreateSshKey => "Upload a new SSH public key.",
            Permission::ViewIsos => "Browse custom ISO images.",
            Permission::ImportIso => "Download an ISO from a URL into the platform.",
            Permission::ViewImages => "Browse custom OS images.",
            Permission::ImportImage => "Download an OS image from a URL into the platform.",
            Permission::ViewBackups => "Browse the automated backup list.",
            Permission::CreateBackup => "Trigger an on-demand backup.",
            Permission::ViewWorkspaces => "See all workspaces and their members.",
            Permission::CreateWorkspace => "Create a new workspace.",
            Permission::EditWorkspace => "Rename or change the description of a workspace.",
            Permission::ManageWorkspaceMembers => "Add or remove users from a workspace and change their roles.",
            Permission::DeleteWorkspace => "Permanently delete a workspace.",
            Permission::ViewUsers => "See the user list and individual user details.",
            Permission::CreateUser => "Add a new system user account.",
            Permission::UpdateUserRole => "Change another user's global role.",
            Permission::ResetUserPassword => "Set a new password for another user.",
            Permission::UpdateUserAbout => "Edit the 'about' description on a user profile.",
            Permission::DeleteUser => "Remove a user account from the system.",
            Permission::ManageAccessAssignments => "Assign or revoke which instances an admin can access.",
        }
    }

    /// Returns every permission that the given global role implicitly grants.
    ///
    /// Rules:
    /// - `owner`  → all permissions.
    /// - `admin`  → instance/resource read+write, workspace management for their
    ///              own workspaces, but NOT user management or global access assignments.
    /// - `viewer` → read-only permissions only.
    pub fn for_role(role: &str) -> Vec<Permission> {
        match role {
            "owner" => Self::all().to_vec(),
            "admin" => vec![
                Permission::ViewInstances,
                Permission::CreateInstance,
                Permission::DeleteInstance,
                Permission::PowerOnInstance,
                Permission::PowerOffInstance,
                Permission::ResetInstance,
                Permission::ChangeInstancePassword,
                Permission::RebuildInstance,
                Permission::ResizeInstance,
                Permission::AddTrafficToInstance,
                Permission::ViewSnapshots,
                Permission::CreateSnapshot,
                Permission::RestoreSnapshot,
                Permission::DeleteSnapshot,
                Permission::ViewFloatingIps,
                Permission::CreateFloatingIp,
                Permission::UpdateFloatingIp,
                Permission::ReleaseFloatingIp,
                Permission::ViewSshKeys,
                Permission::CreateSshKey,
                Permission::ViewIsos,
                Permission::ImportIso,
                Permission::ViewImages,
                Permission::ImportImage,
                Permission::ViewBackups,
                Permission::CreateBackup,
                Permission::ViewWorkspaces,
                Permission::CreateWorkspace,
                Permission::EditWorkspace,
                Permission::ManageWorkspaceMembers,
                Permission::DeleteWorkspace,
            ],
            "viewer" => vec![
                Permission::ViewInstances,
                Permission::ViewSnapshots,
                Permission::ViewFloatingIps,
                Permission::ViewSshKeys,
                Permission::ViewIsos,
                Permission::ViewImages,
                Permission::ViewBackups,
                Permission::ViewWorkspaces,
            ],
            _ => vec![],
        }
    }

    /// Check whether a given global role has this permission.
    pub fn is_allowed_for_role(&self, role: &str) -> bool {
        Self::for_role(role).contains(self)
    }

    /// All defined permissions in a stable display order.
    pub fn all() -> &'static [Permission] {
        &[
            Permission::ViewInstances,
            Permission::CreateInstance,
            Permission::DeleteInstance,
            Permission::PowerOnInstance,
            Permission::PowerOffInstance,
            Permission::ResetInstance,
            Permission::ChangeInstancePassword,
            Permission::RebuildInstance,
            Permission::ResizeInstance,
            Permission::AddTrafficToInstance,
            Permission::ViewSnapshots,
            Permission::CreateSnapshot,
            Permission::RestoreSnapshot,
            Permission::DeleteSnapshot,
            Permission::ViewFloatingIps,
            Permission::CreateFloatingIp,
            Permission::UpdateFloatingIp,
            Permission::ReleaseFloatingIp,
            Permission::ViewSshKeys,
            Permission::CreateSshKey,
            Permission::ViewIsos,
            Permission::ImportIso,
            Permission::ViewImages,
            Permission::ImportImage,
            Permission::ViewBackups,
            Permission::CreateBackup,
            Permission::ViewWorkspaces,
            Permission::CreateWorkspace,
            Permission::EditWorkspace,
            Permission::ManageWorkspaceMembers,
            Permission::DeleteWorkspace,
            Permission::ViewUsers,
            Permission::CreateUser,
            Permission::UpdateUserRole,
            Permission::ResetUserPassword,
            Permission::UpdateUserAbout,
            Permission::DeleteUser,
            Permission::ManageAccessAssignments,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_has_all_permissions() {
        for p in Permission::all() {
            assert!(
                p.is_allowed_for_role("owner"),
                "owner should have permission {:?}",
                p
            );
        }
    }

    #[test]
    fn viewer_cannot_create_resources() {
        assert!(!Permission::CreateInstance.is_allowed_for_role("viewer"));
        assert!(!Permission::DeleteInstance.is_allowed_for_role("viewer"));
        assert!(!Permission::CreateSnapshot.is_allowed_for_role("viewer"));
        assert!(!Permission::CreateWorkspace.is_allowed_for_role("viewer"));
    }

    #[test]
    fn viewer_can_view_resources() {
        assert!(Permission::ViewInstances.is_allowed_for_role("viewer"));
        assert!(Permission::ViewSnapshots.is_allowed_for_role("viewer"));
        assert!(Permission::ViewWorkspaces.is_allowed_for_role("viewer"));
    }

    #[test]
    fn admin_cannot_manage_users() {
        assert!(!Permission::CreateUser.is_allowed_for_role("admin"));
        assert!(!Permission::DeleteUser.is_allowed_for_role("admin"));
        assert!(!Permission::UpdateUserRole.is_allowed_for_role("admin"));
        assert!(!Permission::ManageAccessAssignments.is_allowed_for_role("admin"));
    }

    #[test]
    fn admin_can_manage_workspaces() {
        assert!(Permission::CreateWorkspace.is_allowed_for_role("admin"));
        assert!(Permission::DeleteWorkspace.is_allowed_for_role("admin"));
        assert!(Permission::ManageWorkspaceMembers.is_allowed_for_role("admin"));
    }

    #[test]
    fn unknown_role_has_no_permissions() {
        assert!(Permission::for_role("unknown").is_empty());
        assert!(!Permission::ViewInstances.is_allowed_for_role("unknown"));
    }

    #[test]
    fn all_permissions_have_non_empty_labels_and_descriptions() {
        for p in Permission::all() {
            assert!(!p.label().is_empty(), "{:?} has empty label", p);
            assert!(!p.description().is_empty(), "{:?} has empty description", p);
        }
    }
}
