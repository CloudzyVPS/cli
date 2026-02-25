use askama::Template;
use crate::models::{CurrentUser, Permission};

/// A single row in the permissions reference table.
pub struct PermissionRow {
    pub label: &'static str,
    pub description: &'static str,
    pub owner: bool,
    pub admin: bool,
    pub viewer: bool,
}

#[derive(Template)]
#[template(path = "permissions.html")]
pub struct PermissionsTemplate {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
    pub rows: Vec<PermissionRow>,
}

crate::impl_base_template!(PermissionsTemplate);

impl PermissionsTemplate {
    pub fn build(
        current_user: Option<CurrentUser>,
        api_hostname: String,
        base_url: String,
        flash_messages: Vec<String>,
        has_flash_messages: bool,
    ) -> Self {
        let rows = Permission::all()
            .iter()
            .map(|p| PermissionRow {
                label: p.label(),
                description: p.description(),
                owner: p.is_allowed_for_role("owner"),
                admin: p.is_allowed_for_role("admin"),
                viewer: p.is_allowed_for_role("viewer"),
            })
            .collect();
        PermissionsTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            rows,
        }
    }
}
