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
use crate::api::{load_images, download_image};

#[derive(Deserialize)]
pub struct ImagesQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

#[derive(Deserialize)]
pub struct DownloadImageForm {
    name: String,
    url: String,
    region_id: String,
    format: Option<String>,
    decompress: Option<String>,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    10
}

pub async fn images_list_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<ImagesQuery>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let paginated = load_images(
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
        crate::templates::ImagesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            images: &paginated.images,
            // current_page: paginated.current_page,
            // total_pages: paginated.total_pages,
            // per_page: paginated.per_page,
            total_count: paginated.total_count,
        },
    )
}

pub async fn image_download_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<DownloadImageForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let resp = download_image(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &form.name,
        &form.url,
        &form.region_id,
        form.format,
        form.decompress,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Image download initiated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to download image: {}", detail));
        }
    }
    
    Redirect::to("/images").into_response()
}
