use axum::{
    extract::{Form, Path, State},
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::{AppState, UserRecord, UserRow};
use crate::services::{generate_password_hash, persist_users_file};
use crate::templates::{UsersTemplate, UserDetailTemplate};

use super::helpers::{build_template_globals, ensure_owner, plain_html, TemplateGlobals, render_template};

pub async fn users_list(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let users = state.users.lock().unwrap();
    let mut rows: Vec<UserRow> = users
        .iter()
        .map(|(k, v)| {
            let assigned = if v.assigned_instances.is_empty() {
                String::new()
            } else {
                v.assigned_instances.join(", ")
            };
            UserRow {
                username: k.clone(),
                role: v.role.clone(),
                assigned,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.username.cmp(&b.username));
    drop(users);
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    render_template(&state, &jar, UsersTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            rows: &rows,
        }
    )
}

pub async fn user_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let uname = username.to_lowercase();
    let users = state.users.lock().unwrap();
    let user_row = if let Some(rec) = users.get(&uname) {
        let assigned = if rec.assigned_instances.is_empty() {
            String::new()
        } else {
            rec.assigned_instances.join(", ")
        };
        UserRow {
            username: uname.clone(),
            role: rec.role.clone(),
            assigned,
        }
    } else {
        return plain_html("User not found");
    };
    drop(users);

    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);

    render_template(&state, &jar, UserDetailTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        user: user_row,
    })
}

#[derive(Deserialize)]
pub struct CreateUserForm {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[axum::debug_handler]
pub async fn users_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateUserForm>,
) -> axum::response::Response {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let uname = form.username.trim().to_lowercase();
    if uname.is_empty() || form.password.is_empty() {
        return plain_html("Missing username/password");
    }
    if form.role != "owner" && form.role != "admin" {
        return plain_html("Invalid role");
    }
    {
        let mut users = state.users.lock().unwrap();
        if users.contains_key(&uname) {
            return plain_html("Username exists");
        }
        let hash = generate_password_hash(&form.password);
        users.insert(
            uname.clone(),
            UserRecord {
                password: hash,
                role: form.role.clone(),
                assigned_instances: vec![],
            },
        );
    }
    match persist_users_file(&state.users).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!(%e, "Failed to persist users");
            return plain_html("Failed to persist users");
        }
    }
    axum::response::Redirect::to("/users").into_response()
}

#[derive(Deserialize)]
pub struct ResetPasswordForm {
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username): Path<String>,
    Form(form): Form<ResetPasswordForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    if form.new_password.trim().is_empty() {
        return plain_html("Password cannot be empty");
    }
    let uname = username.to_lowercase();
    {
        let mut users = state.users.lock().unwrap();
        if let Some(rec) = users.get_mut(&uname) {
            rec.password = generate_password_hash(&form.new_password);
        } else {
            return plain_html("User not found");
        }
    }
    match persist_users_file(&state.users).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!(%e, "Failed to persist users");
            return plain_html("Failed to persist users");
        }
    }
    axum::response::Redirect::to(&format!("/users/{}", uname)).into_response()
}

#[derive(Deserialize)]
pub struct UpdateRoleForm {
    pub role: String,
}

pub async fn update_role(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username): Path<String>,
    Form(form): Form<UpdateRoleForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let uname = username.to_lowercase();
    if form.role != "owner" && form.role != "admin" {
        return plain_html("Invalid role");
    }
    {
        let mut users = state.users.lock().unwrap();
        let current_role = match users.get(&uname) {
            Some(r) => r.role.clone(),
            None => return plain_html("User not found"),
        };
        if current_role == "owner" && form.role != "owner" {
            let remaining_owners = users
                .iter()
                .filter(|(n, r)| r.role == "owner" && n.as_str() != uname)
                .count();
            if remaining_owners == 0 {
                return plain_html("At least one owner required");
            }
        }
        if let Some(rec) = users.get_mut(&uname) {
            rec.role = form.role.clone();
        }
    }
    match persist_users_file(&state.users).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!(%e, "Failed to persist users");
            return plain_html("Failed to persist users");
        }
    }
    axum::response::Redirect::to(&format!("/users/{}", uname)).into_response()
}

pub async fn delete_user(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let current = super::helpers::current_username_from_jar(&state, &jar).unwrap_or_default();
    let uname = username.to_lowercase();
    {
        let mut users = state.users.lock().unwrap();
        if uname == current {
            return plain_html("Cannot delete own account");
        }
        if let Some(rec) = users.get(&uname) {
            if rec.role == "owner" {
                let owners = users
                    .iter()
                    .filter(|(name, r)| r.role == "owner" && name.as_str() != uname)
                    .count();
                if owners == 0 {
                    return plain_html("At least one owner required");
                }
            }
        }
        if users.remove(&uname).is_none() {
            return plain_html("User not found");
        }
    }
    match persist_users_file(&state.users).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!(%e, "Failed to persist users");
            return plain_html("Failed to persist users");
        }
    }
    axum::response::Redirect::to("/users").into_response()
}
