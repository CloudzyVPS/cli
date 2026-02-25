use axum::{
    extract::{Form, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::AppState;
use crate::templates::ClockedInstancesTemplate;
use crate::handlers::helpers::{
    build_template_globals, ensure_owner, render_template, TemplateGlobals,
};
use crate::services::persist_clocked_instances_file;

#[derive(Deserialize)]
pub struct UpdateClockedInstancesForm {
    /// Newline- or comma-separated instance IDs
    #[serde(default)]
    pub instance_ids: String,
}

pub async fn clocked_instances_get(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let ids: Vec<String> = {
        let set = state.disabled_instances.lock().unwrap();
        let mut v: Vec<String> = set.iter().cloned().collect();
        v.sort();
        v
    };
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } =
        build_template_globals(&state, &jar);
    render_template(
        &state,
        &jar,
        ClockedInstancesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            clocked_ids: &ids,
        },
    )
}

pub async fn clocked_instances_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<UpdateClockedInstancesForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let new_ids: std::collections::HashSet<String> = form
        .instance_ids
        .split([',', '\n', '\r'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    {
        let mut set = state.disabled_instances.lock().unwrap();
        *set = new_ids.clone();
    }

    if let Err(e) = persist_clocked_instances_file(&new_ids).await {
        tracing::error!(%e, "Failed to persist clocked instances");
    }

    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        entry.push("Clocked instance IDs updated successfully.".into());
    }

    Redirect::to("/clocked-instances").into_response()
}
