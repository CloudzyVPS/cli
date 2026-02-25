use axum::{
    extract::{Form, Path, State, Query},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::{AppState, WorkspaceMember, WorkspaceRecord, WorkspaceRole};
use crate::services::{persist_workspaces_file, slugify, now_iso8601};
use crate::templates::{WorkspacesTemplate, WorkspaceDetailTemplate, WorkspaceInstancesTemplate};

use super::helpers::{
    build_template_globals, ensure_owner, plain_html,
    render_template, TemplateGlobals, current_username_from_jar,
    load_instances_for_user_paginated,
};

// ── List ─────────────────────────────────────────────────────────────────────

/// GET /workspaces — list all workspaces the current user belongs to (or all if owner).
pub async fn workspaces_list(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let username = match current_username_from_jar(&state, &jar) {
        Some(u) => u,
        None => return Redirect::to("/login").into_response(),
    };
    let is_owner = {
        let users = state.users.lock().unwrap();
        users.get(&username).map(|r| r.role == "owner").unwrap_or(false)
    };
    let workspaces = {
        let ws = state.workspaces.lock().unwrap();
        let mut list: Vec<WorkspaceRecord> = ws
            .values()
            .filter(|w| is_owner || w.members.iter().any(|m| m.username == username))
            .cloned()
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    };
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    render_template(
        &state,
        &jar,
        WorkspacesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            workspaces: &workspaces,
        },
    )
}

// ── Create ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateWorkspaceForm {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// POST /workspaces — create a new workspace (owner only).
pub async fn workspace_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateWorkspaceForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return plain_html("Workspace name cannot be empty");
    }
    let slug = slugify(&name);
    if slug.is_empty() {
        return plain_html("Could not generate a valid slug from that name");
    }
    {
        let mut ws = state.workspaces.lock().unwrap();
        if ws.contains_key(&slug) {
            return plain_html("A workspace with that name already exists");
        }
        ws.insert(
            slug.clone(),
            WorkspaceRecord {
                name,
                description: form.description.trim().to_string(),
                slug: slug.clone(),
                created_at: now_iso8601(),
                members: vec![],
                assigned_instances: vec![],
            },
        );
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to(&format!("/workspaces/{}", slug)).into_response()
}

// ── Detail ────────────────────────────────────────────────────────────────────

/// GET /workspaces/:slug — workspace detail page.
pub async fn workspace_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let username = match current_username_from_jar(&state, &jar) {
        Some(u) => u,
        None => return Redirect::to("/login").into_response(),
    };
    let workspace = {
        let ws = state.workspaces.lock().unwrap();
        ws.get(&slug).cloned()
    };
    let workspace = match workspace {
        Some(w) => w,
        None => return plain_html("Workspace not found"),
    };

    // Allow owners and workspace members to view the detail page.
    {
        let users = state.users.lock().unwrap();
        let is_owner = users.get(&username).map(|r| r.role == "owner").unwrap_or(false);
        let is_member = workspace.members.iter().any(|m| m.username == username);
        if !is_owner && !is_member {
            return Redirect::to("/workspaces").into_response();
        }
    }

    // Collect all usernames for the member-add dropdown.
    let all_users: Vec<String> = {
        let users = state.users.lock().unwrap();
        let mut names: Vec<String> = users.keys().cloned().collect();
        names.sort();
        names
    };

    // For owners: load all instances to power the instance assignment checkbox list.
    let all_instances = load_instances_for_user_paginated(&state, "", 0, 0).await;

    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    render_template(
        &state,
        &jar,
        WorkspaceDetailTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            workspace: &workspace,
            all_users: &all_users,
            all_instances: &all_instances.instances,
        },
    )
}

// ── Edit metadata ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditWorkspaceForm {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// POST /workspaces/:slug/edit — rename / redescribe a workspace (owner only).
pub async fn workspace_edit(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
    Form(form): Form<EditWorkspaceForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return plain_html("Workspace name cannot be empty");
    }
    {
        let mut ws = state.workspaces.lock().unwrap();
        if let Some(rec) = ws.get_mut(&slug) {
            rec.name = name;
            rec.description = form.description.trim().to_string();
        } else {
            return plain_html("Workspace not found");
        }
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to(&format!("/workspaces/{}", slug)).into_response()
}

// ── Add member ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddMemberForm {
    pub username: String,
    pub role: String,
}

/// POST /workspaces/:slug/members/add — add a member to a workspace.
pub async fn workspace_add_member(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
    Form(form): Form<AddMemberForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let username = form.username.trim().to_lowercase();
    let role = match WorkspaceRole::from_str(form.role.trim()) {
        Some(r) => r,
        None => return plain_html("Invalid workspace role"),
    };
    // Verify the user exists.
    {
        let users = state.users.lock().unwrap();
        if !users.contains_key(&username) {
            return plain_html("User not found");
        }
    }
    {
        let mut ws = state.workspaces.lock().unwrap();
        if let Some(rec) = ws.get_mut(&slug) {
            // Remove any existing membership for this user then re-add.
            rec.members.retain(|m| m.username != username);
            rec.members.push(WorkspaceMember { username, role });
            rec.members.sort_by(|a, b| a.username.cmp(&b.username));
        } else {
            return plain_html("Workspace not found");
        }
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to(&format!("/workspaces/{}", slug)).into_response()
}

// ── Remove member ─────────────────────────────────────────────────────────────

/// POST /workspaces/:slug/members/:username/remove — remove a member.
pub async fn workspace_remove_member(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((slug, username)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let uname = username.to_lowercase();
    {
        let mut ws = state.workspaces.lock().unwrap();
        if let Some(rec) = ws.get_mut(&slug) {
            rec.members.retain(|m| m.username != uname);
        } else {
            return plain_html("Workspace not found");
        }
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to(&format!("/workspaces/{}", slug)).into_response()
}

// ── Delete workspace ──────────────────────────────────────────────────────────

/// POST /workspaces/:slug/delete — permanently delete a workspace (owner only).
pub async fn workspace_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    // Disallow deletion if current user is not the owner.
    let current = current_username_from_jar(&state, &jar).unwrap_or_default();
    {
        let users = state.users.lock().unwrap();
        if let Some(rec) = users.get(&current) {
            if rec.role != "owner" {
                return plain_html("Only owners can delete workspaces");
            }
        }
    }
    {
        let mut ws = state.workspaces.lock().unwrap();
        if ws.remove(&slug).is_none() {
            return plain_html("Workspace not found");
        }
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to("/workspaces").into_response()
}

// ── Assign instances to workspace ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AssignInstancesForm {
    #[serde(default)]
    pub instances: Vec<String>,
}

/// POST /workspaces/:slug/instances/assign — set which instances belong to this workspace (owner only).
pub async fn workspace_assign_instances(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
    Form(form): Form<AssignInstancesForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    {
        let mut ws = state.workspaces.lock().unwrap();
        if let Some(rec) = ws.get_mut(&slug) {
            let mut ids: Vec<String> = form
                .instances
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            ids.sort();
            ids.dedup();
            rec.assigned_instances = ids;
        } else {
            return plain_html("Workspace not found");
        }
    }
    if let Err(e) = persist_workspaces_file(&state.workspaces).await {
        tracing::error!(%e, "Failed to persist workspaces");
        return plain_html("Failed to save workspace");
    }
    Redirect::to(&format!("/workspaces/{}", slug)).into_response()
}

// ── Workspace instances view ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsPaginationParams {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

fn default_page() -> usize { 1 }
fn default_per_page() -> usize { 10 }

/// GET /workspaces/:slug/instances — instances belonging to this workspace.
/// Members can view; access is restricted to workspace members + owners.
pub async fn workspace_instances(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(slug): Path<String>,
    Query(params): Query<WsPaginationParams>,
) -> impl IntoResponse {
    let username = match current_username_from_jar(&state, &jar) {
        Some(u) => u,
        None => return Redirect::to("/login").into_response(),
    };

    let workspace = {
        let ws = state.workspaces.lock().unwrap();
        ws.get(&slug).cloned()
    };
    let workspace = match workspace {
        Some(w) => w,
        None => return plain_html("Workspace not found"),
    };

    // Only workspace members (or owners) can view workspace instances.
    {
        let users = state.users.lock().unwrap();
        let is_owner = users
            .get(&username)
            .map(|r| r.role == "owner")
            .unwrap_or(false);
        let is_member = workspace.members.iter().any(|m| m.username == username);
        if !is_owner && !is_member {
            return Redirect::to("/workspaces").into_response();
        }
    }

    // Load all instances the current user can access, then filter to only those
    // that are assigned to this workspace.
    let all_user_instances =
        load_instances_for_user_paginated(&state, &username, 0, 0).await;
    let ws_instance_ids: std::collections::HashSet<&str> =
        workspace.assigned_instances.iter().map(|s| s.as_str()).collect();
    let ws_instances: Vec<_> = all_user_instances
        .instances
        .into_iter()
        .filter(|inst| ws_instance_ids.contains(inst.id.as_str()))
        .collect();

    let total_count = ws_instances.len();
    let per_page = params.per_page.max(1);
    let total_pages = (total_count + per_page - 1).max(1) / per_page;
    let current_page = params.page.max(1).min(total_pages.max(1));
    let start = (current_page - 1) * per_page;
    let end = (start + per_page).min(total_count);
    let page_instances = ws_instances[start..end].to_vec();

    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    render_template(
        &state,
        &jar,
        WorkspaceInstancesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            workspace: &workspace,
            instances: &page_instances,
            current_page,
            total_pages,
            per_page,
            total_count,
        },
    )
}
