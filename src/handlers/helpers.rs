use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::http::StatusCode;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::Value;

use crate::api::{
    api_call, load_ssh_keys, load_ssh_keys_paginated, load_regions, load_products, 
    load_instances_for_user, PaginatedInstances, PaginatedSshKeys
};
use crate::models::{AppState, CurrentUser, SshKeyView, Region, ProductView, InstanceView};
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn to_csv(self) -> String {
        match self {
            OneOrMany::One(s) => s,
            OneOrMany::Many(v) => v.join(","),
        }
    }
}

pub fn session_id_from_jar(jar: &CookieJar) -> Option<String> {
    jar.get("session_id").map(|c| c.value().to_string())
}

pub fn current_username_from_jar(state: &AppState, jar: &CookieJar) -> Option<String> {
    let sid = session_id_from_jar(jar)?;
    state.sessions.lock().unwrap().get(&sid).cloned()
}

pub fn take_flash_messages(state: &AppState, jar: &CookieJar) -> Vec<String> {
    let sid = session_id_from_jar(jar);
    if sid.is_none() {
        return vec![];
    }
    let sid = sid.unwrap();
    let mut fs = state.flash_store.lock().unwrap();
    fs.remove(&sid).unwrap_or_else(Vec::new)
}

pub fn resolve_default_endpoint(state: &AppState, username: &str) -> String {
    let users = state.users.lock().unwrap();
    if let Some(rec) = users.get(username) {
        if rec.role == "owner" {
            return "/instances".into();
        }
    }
    "/instances".into()
}

pub fn build_current_user(state: &AppState, jar: &CookieJar) -> Option<CurrentUser> {
    let username = current_username_from_jar(state, jar)?;
    let users = state.users.lock().unwrap();
    let rec = users.get(&username)?;
    Some(CurrentUser {
        username: username.clone(),
        role: rec.role.clone(),
    })
}

#[derive(Default)]
pub struct TemplateGlobals {
    pub current_user: Option<CurrentUser>,
    pub api_hostname: String,
    pub base_url: String,
    pub flash_messages: Vec<String>,
    pub has_flash_messages: bool,
}

pub fn build_template_globals(state: &AppState, jar: &CookieJar) -> TemplateGlobals {
    let current_user = build_current_user(state, jar);
    let flash_messages = take_flash_messages(state, jar);
    let has_flash_messages = !flash_messages.is_empty();
    TemplateGlobals {
        current_user,
        api_hostname: crate::utils::hostname_from_url(&state.api_base_url),
        base_url: state.public_base_url.clone(),
        flash_messages,
        has_flash_messages,
    }
}

pub fn absolute_url_from_state(state: &AppState, path: &str) -> String {
    crate::utils::absolute_url(&state.public_base_url, path)
}

pub fn ensure_owner(state: &AppState, jar: &CookieJar) -> Option<Redirect> {
    let username = current_username_from_jar(state, jar)?;
    let users = state.users.lock().unwrap();
    if let Some(rec) = users.get(&username) {
        if rec.role == "owner" {
            return None;
        }
    }
    Some(Redirect::to("/"))
}

pub async fn load_active_regions(state: &AppState) -> Vec<Region> {
    let (regions, _) = load_regions(&state.client, &state.api_base_url, &state.api_token).await;
    regions
        .into_iter()
        .filter(|region| region.is_active && !region.is_hidden)
        .collect()
}

#[allow(dead_code)]
pub fn ensure_logged_in(state: &AppState, jar: &CookieJar) -> Option<Redirect> {
    if current_username_from_jar(state, jar).is_none() {
        return Some(Redirect::to("/login"));
    }
    None
}

pub fn ensure_admin_or_owner(state: &AppState, jar: &CookieJar) -> Option<Redirect> {
    let username = current_username_from_jar(state, jar)?;
    let users = state.users.lock().unwrap();
    if let Some(rec) = users.get(&username) {
        if rec.role == "owner" || rec.role == "admin" {
            return None;
        }
    }
    Some(Redirect::to("/"))
}

pub fn plain_html<S: AsRef<str>>(s: S) -> Response {
    Html(format!("<!DOCTYPE html><html><body><p>{}</p></body></html>", s.as_ref())).into_response()
}

pub fn render_template<T: askama::Template>(_state: &AppState, _jar: &CookieJar, t: T) -> Response {
    match t.render() {
        Ok(body) => Html(body).into_response(),
        Err(e) => {
            tracing::error!(%e, "Template render error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

static LOGGING_IGNORE_ENDPOINTS: &[&str] = &["/v1/os", "/v1/products"];

pub async fn api_call_wrapper(
    state: &AppState,
    method: &str,
    endpoint: &str,
    data: Option<Value>,
    params: Option<Vec<(String, String)>>,
) -> Value {
    let should_log = !LOGGING_IGNORE_ENDPOINTS.contains(&endpoint);
    if should_log {
        tracing::info!(method, endpoint, ?data, ?params, "API Request");
    }
    let result = api_call(&state.client, &state.api_base_url, &state.api_token, method, endpoint, data, params).await;
    if should_log {
        tracing::info!(response=?result, "API Response");
    }
    result
}

pub fn detail_requires_customer(detail: &str) -> bool {
    detail.to_lowercase().contains("customer id")
}

pub fn extract_customer_id_from_value(value: &Value) -> Option<String> {
    fn recurse(node: &Value) -> Option<String> {
        if let Some(obj) = node.as_object() {
            for key in ["customerId", "customer_id", "id"] {
                if let Some(val) = obj.get(key).and_then(|v| v.as_str()) {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            for key in ["customer", "data"] {
                if let Some(child) = obj.get(key) {
                    if let Some(found) = recurse(child) {
                        return Some(found);
                    }
                }
            }
            for key in ["customers", "items", "records", "results"] {
                if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
                    for entry in arr {
                        if let Some(found) = recurse(entry) {
                            return Some(found);
                        }
                    }
                }
            }
        } else if let Some(arr) = node.as_array() {
            for entry in arr {
                if let Some(found) = recurse(entry) {
                    return Some(found);
                }
            }
        }
        None
    }

    if let Some(data) = value.get("data") {
        if let Some(found) = recurse(data) {
            return Some(found);
        }
    }
    recurse(value)
}

pub async fn fetch_default_customer_id(state: &AppState) -> Option<String> {
    if let Some(existing) = state.default_customer_cache.lock().unwrap().clone() {
        return Some(existing);
    }
    let endpoints = ["/v1/customers", "/v1/profile"];
    for endpoint in endpoints {
        let payload = api_call_wrapper(state, "GET", endpoint, None, None).await;
        if let Some(id) = extract_customer_id_from_value(&payload) {
            let mut cache = state.default_customer_cache.lock().unwrap();
            *cache = Some(id.clone());
            return Some(id);
        }
    }
    None
}

pub async fn load_ssh_keys_api(state: &AppState, customer_id: Option<String>) -> Vec<SshKeyView> {
    load_ssh_keys(&state.client, &state.api_base_url, &state.api_token, customer_id).await
}

pub async fn load_ssh_keys_paginated_wrapper(
    state: &AppState,
    customer_id: Option<String>,
    page: usize,
    per_page: usize,
) -> PaginatedSshKeys {
    load_ssh_keys_paginated(&state.client, &state.api_base_url, &state.api_token, customer_id, page, per_page).await
}

pub async fn load_regions_wrapper(state: &AppState) -> (Vec<Region>, HashMap<String, Region>) {
    load_regions(&state.client, &state.api_base_url, &state.api_token).await
}

pub async fn load_products_wrapper(state: &AppState, region_id: &str) -> Vec<ProductView> {
    load_products(&state.client, &state.api_base_url, &state.api_token, region_id).await
}

#[allow(dead_code)]
pub async fn load_instances_for_user_wrapper(state: &AppState, username: &str) -> Vec<InstanceView> {
    let users_map = state.users.lock().unwrap().clone();
    let result = load_instances_for_user(&state.client, &state.api_base_url, &state.api_token, &users_map, username, 0, 0).await;
    result.instances
}

pub async fn load_instances_for_user_paginated(
    state: &AppState,
    username: &str,
    page: usize,
    per_page: usize,
) -> PaginatedInstances {
    let users_map = state.users.lock().unwrap().clone();
    load_instances_for_user(&state.client, &state.api_base_url, &state.api_token, &users_map, username, page, per_page).await
}
