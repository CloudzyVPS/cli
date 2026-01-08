use axum::{
    extract::{State, Form},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::AppState;
use crate::handlers::helpers::{
    build_template_globals, render_template, TemplateGlobals, ensure_owner,
};
use crate::api::load_backups;

#[derive(Deserialize)]
pub struct CreateBackupForm {
    instance_id: String,
    schedule_frequency: String,
    period_id: String,
}

pub async fn backups_list_get(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let backups = load_backups(
        &state.client,
        &state.api_base_url,
        &state.api_token,
    )
    .await;
    
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = 
        build_template_globals(&state, &jar);
    
    render_template(
        &state,
        &jar,
        crate::templates::BackupsTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            backups: &backups,
        },
    )
}

pub async fn backup_create_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateBackupForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let period_id = form.period_id.parse::<i32>().unwrap_or(7);
    
    let resp = crate::api::create_backup_profile(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &form.instance_id,
        &form.schedule_frequency,
        period_id,
        None,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Backup profile created successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to create backup profile: {}", detail));
        }
    }
    
    Redirect::to("/backups").into_response()
}
