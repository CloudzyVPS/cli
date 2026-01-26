use axum::{
    extract::{State, Query, Form},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::AppState;
use crate::handlers::helpers::{
    build_template_globals, render_template, TemplateGlobals, ensure_owner,
};
use crate::api::{load_isos, download_iso};

#[derive(Deserialize)]
pub struct IsosQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

#[derive(Deserialize)]
pub struct DownloadIsoForm {
    name: String,
    url: String,
    region_id: String,
    #[serde(default)]
    use_virtio: String,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    10
}

pub async fn isos_list_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<IsosQuery>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let paginated = load_isos(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        q.page,
        q.per_page,
    )
    .await;
    
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = 
        build_template_globals(&state, &jar);
    
    render_template(
        &state,
        &jar,
        crate::templates::IsosTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            isos: &paginated.isos,
            total_count: paginated.total_count,
        },
    )
}

pub async fn iso_download_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<DownloadIsoForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let use_virtio = form.use_virtio == "true" || form.use_virtio == "1" || form.use_virtio == "on";
    
    let resp = download_iso(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &form.name,
        &form.url,
        &form.region_id,
        use_virtio,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("ISO download initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to download ISO: {}", detail));
        }
    }
    
    Redirect::to("/isos").into_response()
}
