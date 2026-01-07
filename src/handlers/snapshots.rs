use axum::{
    extract::{State, Path, Query, Form},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::AppState;
use crate::handlers::helpers::{
    build_template_globals, current_username_from_jar,
    render_template, TemplateGlobals, ensure_owner,
};
use crate::api::{load_snapshots, create_snapshot, get_snapshot, delete_snapshot, restore_snapshot};
use crate::services::instance_service::enforce_instance_access;

#[derive(Deserialize)]
pub struct SnapshotsQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
    instance_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateSnapshotForm {
    instance_id: String,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    10
}

pub async fn snapshots_list_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<SnapshotsQuery>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let paginated = load_snapshots(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        q.instance_id.clone(),
        q.page,
        q.per_page,
    )
    .await;
    
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = 
        build_template_globals(&state, &jar);
    
    render_template(
        &state,
        &jar,
        crate::templates::SnapshotsTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            snapshots: &paginated.snapshots,
            current_page: paginated.current_page,
            total_pages: paginated.total_pages,
            per_page: paginated.per_page,
            total_count: paginated.total_count,
            filter_instance_id: q.instance_id,
        },
    )
}

pub async fn snapshot_detail_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(snapshot_id): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let payload = get_snapshot(&state.client, &state.api_base_url, &state.api_token, &snapshot_id).await;
    
    let mut snapshot_data = None;
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            snapshot_data = Some(data.clone());
        }
    }
    
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = 
        build_template_globals(&state, &jar);
    
    render_template(
        &state,
        &jar,
        crate::templates::SnapshotDetailTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            snapshot_id,
            snapshot_data,
        },
    )
}

pub async fn snapshot_create_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateSnapshotForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    // Check access to instance
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &form.instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    
    let resp = create_snapshot(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &form.instance_id,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Snapshot creation initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Snapshot creation failed: {}", detail));
        }
    }
    
    Redirect::to("/snapshots").into_response()
}

pub async fn snapshot_delete_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(snapshot_id): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let resp = delete_snapshot(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &snapshot_id,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Snapshot deleted successfully.".into());
            return Redirect::to("/snapshots").into_response();
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Snapshot deletion failed: {}", detail));
            return Redirect::to(&format!("/snapshots/{}", snapshot_id)).into_response();
        }
    }
    
    Redirect::to("/snapshots").into_response()
}

pub async fn snapshot_restore_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(snapshot_id): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let resp = restore_snapshot(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &snapshot_id,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Snapshot restore initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Snapshot restore failed: {}", detail));
        }
    }
    
    Redirect::to(&format!("/snapshots/{}", snapshot_id)).into_response()
}
