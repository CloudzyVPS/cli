use axum::{
    extract::{Path, State, Form},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use std::collections::HashSet;
use serde::Deserialize;

use crate::models::{AppState, AdminView, InstanceCheckbox, InstanceView};
use crate::templates::AccessTemplate;
use crate::handlers::helpers::{
    build_template_globals, ensure_owner, render_template, TemplateGlobals,
    api_call_wrapper, plain_html,
};
use crate::services::persist_users_file;

#[derive(Deserialize)]
pub struct UpdateAccessForm {
    #[serde(default)]
    #[serde(rename = "instances")]
    instances: Vec<String>,
}

// Access management (owner only): list admins and assign instances

pub async fn access_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    // Load instances
    let payload = api_call_wrapper(&state, "GET", "/v1/instances", None, None).await;
    let mut list: Vec<InstanceView> = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        let candidates = if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            arr.clone()
        } else if let Some(data) = payload.get("data").and_then(|d| d.as_object()) {
            if let Some(arr) = data.get("instances").and_then(|i| i.as_array()) {
                arr.clone()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        for item in candidates {
            if let Some(obj) = item.as_object() {
                let id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| obj.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()))
                    .unwrap_or("?".into());
                let hostname = obj
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no hostname)")
                    .to_string();
                let status = obj
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let status_display = crate::utils::format_status(&status);
                let region = obj
                    .get("region")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                list.push(InstanceView { 
                    id, 
                    hostname, 
                    region,
                    status,
                    status_display,
                    vcpu_count_display: "—".into(),
                    ram_display: "—".into(),
                    disk_display: "—".into(),
                    main_ip: None,
                    os: None,
                });
            }
        }
    }
    // Collect admins
    let users = state.users.lock().unwrap();
    let mut admins: Vec<AdminView> = users
        .iter()
        .filter(|(_, rec)| rec.role == "admin")
        .map(|(u, rec)| {
            let assigned: HashSet<&str> =
                rec.assigned_instances.iter().map(|s| s.as_str()).collect();
            let rows = list
                .iter()
                .map(|inst| {
                    let checked = assigned.contains(inst.id.as_str());
                    InstanceCheckbox {
                        id: inst.id.clone(),
                        hostname: inst.hostname.clone(),
                        checked,
                    }
                })
                .collect();
            AdminView {
                username: u.clone(),
                instances: rows,
            }
        })
        .collect();
    admins.sort_by(|a, b| a.username.cmp(&b.username));
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    render_template(&state, &jar, AccessTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, admins: &admins })
}

pub async fn update_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(username): Path<String>,
    Form(form): Form<UpdateAccessForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let uname = username.to_lowercase();
    {
        let mut users = state.users.lock().unwrap();
        if let Some(rec) = users.get_mut(&uname) {
            if rec.role != "admin" {
                return plain_html("Target user not admin");
            }
            // Normalize and dedupe
            let mut normalized: Vec<String> = form
                .instances
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            normalized.sort();
            normalized.dedup();
            rec.assigned_instances = normalized;
        } else {
            return plain_html("Admin not found");
        }
    }
    
    if let Err(e) = persist_users_file(&state.users).await {
        tracing::error!(%e, "Failed to persist users");
        return plain_html("Failed to persist users");
    }

    Redirect::to("/access").into_response()
}
