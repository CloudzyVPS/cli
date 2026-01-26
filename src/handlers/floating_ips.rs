use axum::{
    extract::{State, Path, Query, Form},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::AppState;
use crate::handlers::helpers::{
    build_template_globals, render_template, TemplateGlobals, ensure_owner, load_active_regions,
};
use crate::api::{load_floating_ips, create_floating_ips, update_floating_ip, release_floating_ip};

#[derive(Deserialize)]
pub struct FloatingIpsQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_per_page")]
    per_page: usize,
}

#[derive(Deserialize)]
pub struct CreateFloatingIpForm {
    region_id: String,
    count: String,
}

#[derive(Deserialize)]
pub struct UpdateFloatingIpForm {
    auto_renew: Option<String>,
    customer_note: Option<String>,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    10
}

pub async fn floating_ips_list_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<FloatingIpsQuery>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let paginated = load_floating_ips(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        q.page,
        q.per_page,
    )
    .await;
    let regions = load_active_regions(&state).await;
    
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = 
        build_template_globals(&state, &jar);
    
    render_template(
        &state,
        &jar,
        crate::templates::FloatingIpsTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            floating_ips: &paginated.floating_ips,
            current_page: paginated.current_page,
            total_pages: paginated.total_pages,
            per_page: paginated.per_page,
            total_count: paginated.total_count,
            regions: &regions,
        },
    )
}

pub async fn floating_ip_create_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateFloatingIpForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let count = form.count.parse::<i32>().unwrap_or(1).max(1).min(5);
    
    let resp = create_floating_ips(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &form.region_id,
        count,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push(format!("{} floating IP(s) created successfully.", count));
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to create floating IPs: {}", detail));
        }
    }
    
    Redirect::to("/floating-ips").into_response()
}

pub async fn floating_ip_update_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(ip_id): Path<String>,
    Form(form): Form<UpdateFloatingIpForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let auto_renew = form.auto_renew.as_ref().map(|s| s == "true" || s == "1" || s == "on");
    
    let resp = update_floating_ip(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &ip_id,
        auto_renew,
        form.customer_note,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Floating IP updated successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to update floating IP: {}", detail));
        }
    }
    
    Redirect::to("/floating-ips").into_response()
}

pub async fn floating_ip_release_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(ip_id): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    
    let resp = release_floating_ip(
        &state.client,
        &state.api_base_url,
        &state.api_token,
        &ip_id,
    )
    .await;
    
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Floating IP released successfully.".into());
        } else {
            let detail = resp.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Failed to release floating IP: {}", detail));
        }
    }
    
    Redirect::to("/floating-ips").into_response()
}
