mod config;
mod models;
mod services;
mod utils;
mod api;
mod templates;
mod handlers;

use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderValue;
use tower::ServiceBuilder;
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::process;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use axum_extra::extract::cookie::CookieJar;

use config::{DEFAULT_HOST, DEFAULT_PORT};
use models::{UserRecord, AppState, AddTrafficForm, ChangeOsForm, ResizeForm, ProductView, OsItem, InstanceView, SshKeyView, AdminView, InstanceCheckbox, Region};
use services::{generate_password_hash, load_users_from_file, persist_users_file, simple_instance_action, enforce_instance_access};
use api::{api_call, load_regions, load_products, load_os_list, load_instances_for_user, load_ssh_keys};
use templates::*;
use handlers::helpers::{
    build_template_globals, current_username_from_jar,
    ensure_owner, ensure_logged_in, plain_html, TemplateGlobals, render_template,
};
use std::collections::HashSet;
// No-op logging ignore endpoint list
static LOGGING_IGNORE_ENDPOINTS: &[&str] = &["/v1/os", "/v1/products", "/os", "/products"];

async fn build_state_from_env(env_file: Option<&str>) -> AppState {
    config::load_env_file(env_file);
    let users = load_users_from_file().await;
    let disabled_instances = std::sync::Arc::new(config::get_disabled_instance_ids());
    
    AppState {
        users,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        flash_store: Arc::new(Mutex::new(HashMap::new())),
        default_customer_cache: Arc::new(Mutex::new(None)),
        api_base_url: config::get_api_base_url(),
        api_token: config::get_api_token(),
        public_base_url: config::get_public_base_url(),
        client: reqwest::Client::new(),
        disabled_instances,
    }
}

// Global template context injected into most page templates
// (already implemented via build_template_globals/TemplateGlobals)
fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::auth::root_get))
        .route("/login", get(handlers::auth::login_get).post(handlers::auth::login_post))
        .route("/logout", post(handlers::auth::logout_post))
        .route("/users", get(handlers::users::users_list).post(handlers::users::users_create))
        .route("/users/:username/reset-password", post(handlers::users::reset_password))
        .route("/users/:username/role", post(handlers::users::update_role))
        .route("/users/:username/delete", post(handlers::users::delete_user))
        .route("/access", get(access_get))
        .route("/access/:username", post(update_access))
        .route("/ssh-keys", get(ssh_keys_get).post(ssh_keys_post))
        .route("/instances", get(instances_real))
        .route("/regions", get(handlers::catalog::regions_get))
        .route("/products", get(handlers::catalog::products_get))
        .route("/os", get(handlers::catalog::os_get))
        .route("/applications", get(handlers::catalog::applications_get))
        .route("/create/step-1", get(handlers::wizard::create_step_1))
        .route("/create/step-2", get(handlers::wizard::create_step_2))
        .route("/create/step-3", get(handlers::wizard::create_step_3))
        .route("/create/step-4", get(handlers::wizard::create_step_4))
        .route("/create/step-5", get(handlers::wizard::create_step_5))
        .route("/create/step-6", get(handlers::wizard::create_step_6))
        .route(
            "/create/step-7",
            get(handlers::wizard::create_step_7_get).post(handlers::wizard::create_step_7_post),
        )
        .route("/create/result", get(handlers::wizard::create_step_8))
        .route("/instance/:instance_id", get(instance_detail))
        .route("/instance/:instance_id/delete", get(instance_delete_get).post(instance_delete))
        .route("/instance/:instance_id/poweron", get(instance_poweron_get).post(instance_poweron_post))
        .route("/instance/:instance_id/poweroff", get(instance_poweroff_get).post(instance_poweroff_post))
        .route("/instance/:instance_id/reset", get(instance_reset_get).post(instance_reset_post))
        .route(
            "/instance/:instance_id/change-pass",
            get(instance_change_pass_get).post(instance_change_pass_post),
        )
        .route("/instance/:instance_id/change-os", get(instance_change_os_get).post(instance_change_os_post))
        .route("/instance/:instance_id/resize", get(instance_resize_get).post(instance_resize_post))
        .route(
            "/instance/:instance_id/subscription-refund",
            get(instance_subscription_refund),
        )
        .route(
            "/instance/:instance_id/add-traffic",
            post(instance_add_traffic),
        )
        .route(
            "/bulk-subscription-refund",
            get(bulk_subscription_refund_get).post(bulk_subscription_refund),
        )
        // Serve static files with cache-control header to avoid reloading stylesheets on each request
        .nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new("static")),
        )
        .with_state(state)
}

async fn start_server(state: AppState, host: &str, port: u16) {
    let addr: SocketAddr = match format!("{}:{}", host, port).parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(%e, "Invalid host/port format");
            eprintln!("Invalid host/port format: {}", e);
            process::exit(1);
        }
    };
    let app = build_app(state.clone());
    tracing::info!(%addr, "Starting Zyffiliate Rust server");
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            // Run the server and log any errors (do not panic with unwrap()).
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!(%e, "Server encountered an error while running");
                eprintln!("Server error: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            tracing::error!(%e, "Failed to bind to address; is the port already in use?");
            eprintln!("Failed to bind to {}: {}\nPlease stop any process using this port, or start the server with a different --port value.", addr, e);
            process::exit(1);
        }
    }
}

async fn load_instances_for_user_wrapper(state: &AppState, username: &str) -> Vec<InstanceView> {
    let users_map = state.users.lock().unwrap().clone();
    load_instances_for_user(&state.client, &state.api_base_url, &state.api_token, &users_map, username).await
}

// Wrapper for API calls with optional logging (used by main.rs handlers)
async fn api_call_wrapper(
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

async fn load_products_wrapper(state: &AppState, region_id: &str) -> Vec<ProductView> {
    load_products(&state.client, &state.api_base_url, &state.api_token, region_id).await
}

async fn load_regions_wrapper(state: &AppState) -> (Vec<Region>, HashMap<String, Region>) {
    load_regions(&state.client, &state.api_base_url, &state.api_token).await
}

async fn load_os_list_wrapper(state: &AppState) -> Vec<OsItem> {
    load_os_list(&state.client, &state.api_base_url, &state.api_token).await
}

// Now using UserRow from models

async fn instances_real(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let Some(username) = current_username_from_jar(&state, &jar) else {
        return Redirect::to("/login").into_response();
    };
    let list = load_instances_for_user_wrapper(&state, &username).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, InstancesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instances: &list,
        },
    )
}

// Access management (owner only): list admins and assign instances

async fn access_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    // Load instances
    let payload = api_call_wrapper(&state, "GET", "/v1/instances", None, None).await;
    let mut list: Vec<InstanceView> = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(items) = payload
            .get("data")
            .and_then(|d| d.get("instances"))
            .and_then(|arr| arr.as_array())
        {
            for item in items {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|n| n.to_string())
                    .or_else(|| {
                        item.get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "?".into());
                let hostname = item
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(no hostname)".into());
                let status = item
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "?".into());
                let region = item
                    .get("region")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                list.push(InstanceView { 
                    id, 
                    hostname, 
                    region,
                    status,
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

#[derive(Deserialize)]
struct UpdateAccessForm {
    #[serde(rename = "instances")]
    instances: Vec<String>,
}

async fn update_access(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(username): axum::extract::Path<String>,
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
// SSH Keys CRUD (owner only)

#[derive(Deserialize)]
struct SshKeysForm {
    action: Option<String>,
    name: Option<String>,
    public_key: Option<String>,
    ssh_key_id: Option<String>,
}

fn detail_requires_customer(detail: &str) -> bool {
    detail.to_lowercase().contains("customer id")
}

fn extract_customer_id_from_value(value: &Value) -> Option<String> {
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

async fn ssh_keys_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let customer_id = if let Some(id) = q.get("customer_id").cloned() {
        Some(id)
    } else {
        fetch_default_customer_id(&state).await
    };
    let keys = load_ssh_keys_api(&state, customer_id.clone()).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, SshKeysTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            ssh_keys: &keys,
            customer_id,
        },
    )
}

async fn ssh_keys_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<SshKeysForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let action = form.action.clone().unwrap_or_else(|| "create".into());
    if action == "delete" {
        let key_id_raw = form.ssh_key_id.clone().unwrap_or_default();
        if !key_id_raw.chars().all(|c| c.is_ascii_digit()) {
            return plain_html("Invalid key id");
        }
        let endpoint = format!("/v1/ssh-keys/{}", key_id_raw);
        let payload = api_call_wrapper(&state, "DELETE", &endpoint, None, None).await;
        if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
            if let Some(detail) = payload.get("detail").and_then(|d| d.as_str()) {
                if detail_requires_customer(detail) {
                    if let Some(cid) = fetch_default_customer_id(&state).await {
                        let _ = api_call_wrapper(
                            &state,
                            "DELETE",
                            &endpoint,
                            None,
                            Some(vec![("customerId".into(), cid)]),
                        )
                        .await;
                    }
                }
            }
        }
        return Redirect::to("/ssh-keys").into_response();
    }
    let name = form.name.clone().unwrap_or_default().trim().to_string();
    let public_key = form
        .public_key
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() || public_key.is_empty() {
        return plain_html("Provide name and public key");
    }
    let mut body = serde_json::json!({"name": name, "publicKey": public_key});
    let payload = api_call_wrapper(&state, "POST", "/v1/ssh-keys", Some(body.clone()), None).await;
    if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
        if let Some(detail) = payload.get("detail").and_then(|d| d.as_str()) {
            if detail_requires_customer(detail) {
                if let Some(cid) = fetch_default_customer_id(&state).await {
                    body["customerId"] = Value::String(cid.clone());
                    let _ = api_call_wrapper(&state, "POST", "/v1/ssh-keys", Some(body), None).await;
                }
            }
        }
    }
    Redirect::to("/ssh-keys").into_response()
}

// Regions are rendered using `templates/regions.html` (path-based Askama template)

// Products are rendered using `templates/products.html` (path-based Askama template)

// OS catalog is rendered using `templates/os.html` (path-based Askama template)

// Applications are rendered using `templates/applications.html` (path-based Askama template)

async fn instance_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let _json = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into());
    // Collect nice key-value pair details we want to display rather than raw JSON
    let mut details: Vec<(String, String)> = Vec::new();
    let mut hostname = "(no hostname)".to_string();
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            hostname = data
                .get("hostname")
                .and_then(|v| v.as_str())
                .unwrap_or("(no hostname)")
                .to_string();
            details.push(("Hostname".into(), hostname.clone()));
            let status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            details.push(("Status".into(), status));
            let region = data
                .get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            details.push(("Region".into(), region.clone()));
            let class = data
                .get("class")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            details.push(("Instance class".into(), class));
            let product_id = data
                .get("productId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(pid) = product_id.clone() {
                // Try to resolve product name using region-scoped product listing
                let product_name = if !region.is_empty() && !pid.is_empty() {
                    let products = load_products_wrapper(&state, &region).await;
                    products
                        .into_iter()
                        .find(|p| p.id == pid)
                        .map(|p| p.name)
                        .unwrap_or(pid.clone())
                } else {
                    pid.clone()
                };
                details.push(("Product".into(), product_name));
            }
            let vcpu = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|v| v.to_string());
            if let Some(x) = vcpu { details.push(("vCPU".into(), x)); }
            let ram = data.get("ram").and_then(|v| v.as_i64()).map(|v| format!("{} MB", v));
            if let Some(x) = ram { details.push(("RAM".into(), x)); }
            let disk = data.get("disk").and_then(|v| v.as_i64()).map(|v| format!("{} GB", v));
            if let Some(x) = disk { details.push(("Disk".into(), x)); }
            let ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(x) = ip { details.push(("IPv4".into(), x)); }
            let ip6 = data.get("mainIpv6").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(x) = ip6 { details.push(("IPv6".into(), x)); }
            if let Some(os_obj) = data.get("os") {
                let os_name = os_obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| os_obj.get("id").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                if !os_name.is_empty() { details.push(("OS".into(), os_name)); }
            }
            if let Some(inserted) = data.get("insertedAt").and_then(|v| v.as_str()).map(|s| s.to_string()) {
                details.push(("Created".into(), inserted));
            }
            if let Some(features) = data.get("features").and_then(|v| v.as_array()) {
                let mut features_list = Vec::new();
                for item in features { if let Some(s) = item.as_str() { features_list.push(s.to_string()); } }
                if !features_list.is_empty() { details.push(("Features".into(), features_list.join(", "))); }
            }
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, InstanceDetailTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instance_id: instance_id.clone(),
            hostname,
            details,
            is_disabled: state.is_instance_disabled(&instance_id),
        },
    )
}


// immediate instance_poweron action removed; use confirmation GET/POST handlers instead

async fn instance_poweron_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.vcpu_count_display = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            instance.ram_display = data.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
            instance.disk_display = data.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, PowerOnInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, is_disabled: state.is_instance_disabled(&instance_id) })
}

// POST handler for poweron
async fn instance_poweron_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "poweron", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}
// immediate instance_poweroff action removed; use confirmation GET/POST handlers instead
// immediate instance_reset action removed; use confirmation GET/POST handlers instead

// Render confirm page for delete (GET) and perform delete (POST implemented as instance_delete)
async fn instance_delete_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.vcpu_count_display = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            instance.ram_display = data.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
            instance.disk_display = data.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.vcpu_count_display = data.get("vcpuCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            instance.ram_display = data.get("ram").and_then(|v| v.as_i64()).map(|n| format!("{} MB", n)).unwrap_or_else(|| "—".into());
            instance.disk_display = data.get("disk").and_then(|v| v.as_i64()).map(|n| format!("{} GB", n)).unwrap_or_else(|| "—".into());
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, DeleteInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, is_disabled: state.is_instance_disabled(&instance_id) })
}

// Render confirm page for poweroff (GET) and perform poweroff (POST handler below)
async fn instance_poweroff_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, PowerOffInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, is_disabled: state.is_instance_disabled(&instance_id) })
}

// POST handler for poweroff
async fn instance_poweroff_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "poweroff", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

// Render confirm page for reset
async fn instance_reset_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, ResetInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, is_disabled: state.is_instance_disabled(&instance_id) })
}

// POST handler for reset
async fn instance_reset_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let _ = simple_instance_action(&state, "reset", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

// GET confirm page for change password
async fn instance_change_pass_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, ChangePassInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, new_password: None, is_disabled: state.is_instance_disabled(&instance_id) })
}

// POST handler for change-pass; display generated password in template
async fn instance_change_pass_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}/change-pass", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}/change-pass", instance_id);
    let payload = api_call_wrapper(&state, "POST", &endpoint, None, None).await;
    let new_password = payload.get("data").and_then(|d| d.get("password")).and_then(|v| v.as_str()).map(|s| s.to_string());
    // Fetch instance details for rendering
    let get_endpoint = format!("/v1/instances/{}", instance_id);
    let payload2 = api_call_wrapper(&state, "GET", &get_endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload2.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, ChangePassInstanceTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, new_password, is_disabled: state.is_instance_disabled(&instance_id) })
}

async fn instance_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "DELETE", &endpoint, None, None).await;
    // Optionally set flash message for success or failure
    if let Some(sid) = jar.get("session_id") {
        let mut flashes = state.flash_store.lock().unwrap();
        let entry = flashes.entry(sid.value().to_string()).or_default();
        if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
            entry.push("Instance deleted successfully.".into());
            return Redirect::to("/instances").into_response();
        } else {
            let detail = payload.get("detail").and_then(|d| d.as_str()).unwrap_or("Unknown error");
            entry.push(format!("Delete failed: {}", detail));
            return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
        }
    }
    // If no session-id in cookie, still redirect based on result
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        Redirect::to("/instances").into_response()
    } else {
        Redirect::to(&format!("/instance/{}", instance_id)).into_response()
    }
}


async fn instance_add_traffic(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
    Form(form): Form<AddTrafficForm>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
    }
    if let Ok(amount) = form.traffic_amount.parse::<f64>() {
        if amount > 0.0 {
            let endpoint = format!("/v1/instances/{}/add-traffic", instance_id);
            let payload = serde_json::json!({"amount": amount});
            let _ = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
        }
    }
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}


async fn instance_change_os_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            if let Some(os_obj) = data.get("os").and_then(|v| v.as_object()) {
                instance.os = Some(OsItem {
                    id: os_obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: os_obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    family: os_obj.get("family").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    arch: os_obj.get("arch").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    min_ram: os_obj.get("minRam").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    is_default: os_obj.get("isDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let os_list = load_os_list_wrapper(&state).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, ChangeOsTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, os_list: &os_list, is_disabled: state.is_instance_disabled(&instance_id) })
}

async fn instance_change_os_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
    Form(form): Form<ChangeOsForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}/change-os", instance_id)).into_response();
    }
    if form.os_id.trim().is_empty() {
        return Redirect::to(&format!("/instance/{}/change-os", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}/change-os", instance_id);
    let payload = serde_json::json!({"osId": form.os_id});
    let _ = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}


async fn instance_resize_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    let mut instance = InstanceView { id: instance_id.clone(), hostname: "(no hostname)".into(), region: "".into(), main_ip: None, status: "".into(), vcpu_count_display: "—".into(), ram_display: "—".into(), disk_display: "—".into(), os: None };
    if let Some(obj) = payload.as_object() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            instance.hostname = data.get("hostname").and_then(|v| v.as_str()).unwrap_or(&instance.hostname).to_string();
            instance.region = data.get("region").and_then(|v| v.as_str()).unwrap_or("").to_string();
            instance.main_ip = data.get("mainIp").and_then(|v| v.as_str()).map(|s| s.to_string());
            instance.status = data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    let (regions, _map) = load_regions_wrapper(&state).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, ResizeTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, instance, regions: &regions, is_disabled: state.is_instance_disabled(&instance_id) })
}

async fn instance_resize_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
    Form(form): Form<ResizeForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_logged_in(&state, &jar) {
        return r.into_response();
    }
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if state.is_instance_disabled(&instance_id) {
        if let Some(sid) = jar.get("session_id") {
            let mut flashes = state.flash_store.lock().unwrap();
            let entry = flashes.entry(sid.value().to_string()).or_default();
            entry.push("Actions are disabled for this instance.".into());
        }
        return Redirect::to(&format!("/instance/{}/resize", instance_id)).into_response();
    }
    let endpoint = format!("/v1/instances/{}/resize", instance_id);
    let mut payload = serde_json::json!({"type": form.r#type});
    if form.r#type.to_uppercase() == "FIXED" {
        if let Some(pid) = form.product_id {
            payload["productId"] = Value::from(pid);
        }
    } else {
        let mut obj = serde_json::Map::new();
        if let Some(rid) = form.region_id { obj.insert("regionId".into(), Value::from(rid)); }
        if let Some(cpu) = form.cpu { if let Ok(n) = cpu.parse::<i64>() { obj.insert("cpu".into(), Value::from(n)); }}
        if let Some(ram) = form.ram_in_gb { if let Ok(n) = ram.parse::<i64>() { obj.insert("ramInGB".into(), Value::from(n)); }}
        if let Some(disk) = form.disk_in_gb { if let Ok(n) = disk.parse::<i64>() { obj.insert("diskInGB".into(), Value::from(n)); }}
        if let Some(bw) = form.bandwidth_in_tb { if let Ok(n) = bw.parse::<i64>() { obj.insert("bandwidthInTB".into(), Value::from(n)); }}
        if !obj.is_empty() {
            payload["resource"] = Value::Object(obj);
        }
    }
    let _ = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

// Subscription refund
async fn instance_subscription_refund(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, current_username_from_jar(&state, &jar).as_deref(), &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}/subscription-refund", instance_id);
    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
    Html(format!("<html><body><h1>Refund {}</h1><pre>{}</pre><p><a href='/instance/{}'>Back</a></p></body></html>", instance_id, serde_json::to_string_pretty(&payload).unwrap_or("{}" .into()), instance_id)).into_response()
}

// Bulk subscription refund (owner)
// Bulk refund page is rendered via `templates/bulk_refund.html` (path-based Askama template)

#[derive(Deserialize)]
struct BulkRefundForm {
    ids: String,
}

async fn bulk_subscription_refund(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<BulkRefundForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let ids: Vec<String> = form
        .ids
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let payload = serde_json::json!({"ids": ids});
    let resp = api_call_wrapper(
        &state,
        "POST",
        "/v1/instances/bulk-subscription-refund",
        Some(payload),
        None,
    )
    .await;
    Html(format!("<html><body><h1>Bulk Refund Result</h1><pre>{}</pre><p><a href='/instances'>Back</a></p></body></html>", serde_json::to_string_pretty(&resp).unwrap_or("{}" .into()))).into_response()
}
async fn bulk_subscription_refund_get(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    render_template(&state, &jar, BulkRefundTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
        })
}

#[derive(Parser)]
#[command(
    name = "zy",
     author,
     version,
    about = "Zy command-line tool",
    long_about = r#"Zy — control and manage your Cloudzy services right from home.

This tool surfaces a small set of commands to run the server, validate configuration, manage local users and manage instances through the API. Use the `--env-file` option or environment variables to provide API credentials.

Examples:
  1) Build & run (dev):
      cargo run -- serve --host 127.0.0.1 --port 5000
  2) Build a release binary:
      cargo build --release
    3) Manage instances:
          zy instances list
          zy instances show 12345
"#, 
    after_help = "Use `zy <subcommand> --help` to get subcommand specific options and usage examples."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web server
    Serve {
        /// Host to bind to
        #[arg(long, default_value_t = String::from(DEFAULT_HOST))]
        host: String,
        /// Port to bind to
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Path to .env file
        #[arg(long)]
        env_file: Option<String>,
    },
    /// Validate configuration (env vars / API credentials)
    #[command(about = "Validate configuration and ensure API connectivity.", long_about = "Validate environment variables required for the Zy server, and optionally validate the configured API token by attempting to fetch regions from the remote API.")]
    CheckConfig { env_file: Option<String> },
    /// Manage local users (users.json)
    Users {
        #[command(subcommand)]
        sub: UserCommands,
    },
    /// Manage instances via the configured API
    #[command(about = "Manage compute instances via the API (list, show, power, delete, etc.)", long_about = "These commands perform the same actions that the web UI's instance actions perform; they make API requests using the current API configuration and token. Be careful with commands that mutate state (delete, reset). Use `--help` on a subcommand for detailed examples.")]
    Instances {
        #[command(subcommand)]
        sub: InstanceCommands,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    #[command(about = "List current users", long_about = "Enumerate users stored in users.json (username, role, assigned_instances).")]
    List,
    #[command(about = "Add a new user", long_about = "Add a user with a role (owner|admin). The password will be hashed and saved to users.json.")]
    Add {
        username: String,
        password: String,
        role: String,
    },
    /// Add a new owner user (use --force to overwrite existing owner user(s))
    #[command(about = "Add an owner user", long_about = "Create a new owner user. Use --force to overwrite an existing owner user or create another owner.")]
    AddOwner {
        username: String,
        password: String,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    #[command(about = "Reset a user's password", long_about = "Set a new password for an existing user; password will be hashed.")]
    ResetPassword {
        username: String,
        password: String,
    },
}

#[derive(Subcommand)]
enum InstanceCommands {
    /// List instances (optional --username to filter)
    #[command(about = "List instances", long_about = "List instances the configured API user may access. Provide `--username` to filter instances assigned to a local user.")]
    List {
        /// Optional username to filter instances by assigned user (use empty to list all)
        #[arg(long)]
        username: Option<String>,
    },
    /// Show instance details
    #[command(about = "Show instance details", long_about = "Show the raw JSON payload returned by the API for an instance ID.")]
    Show { instance_id: String },
    /// Power on an instance
    #[command(about = "Power on an instance", long_about = "Request an asynchronous power-on operation for an instance; the API may perform the action asynchronously.")]
    PowerOn { instance_id: String },
    /// Power off an instance
    #[command(about = "Power off an instance", long_about = "Request an asynchronous power-off operation for an instance; follow up with `show` to confirm state.")]
    PowerOff { instance_id: String },
    /// Reset an instance
    #[command(about = "Reset an instance", long_about = "Request an immediate reset/reboot of the instance. This is destructive to running state but usually preserves disks.")]
    Reset { instance_id: String },
    /// Delete an instance
    #[command(about = "Delete an instance", long_about = "Permanently delete an instance. Use with care and confirm `id` and `username` if necessary.")]
    Delete { instance_id: String },
    /// Change the instance password (prints the generated password)
    #[command(about = "Change root/console password", long_about = "Generate and set a new root/console password for an instance and print the generated value (if API returns it).")]
    ChangePass { instance_id: String },
    /// Change the instance OS
    #[command(about = "Change the instance OS", long_about = "Trigger an OS distribution and image change. Provide a valid `os_id` from the remote API.")]
    ChangeOs { instance_id: String, os_id: String },
    /// Resize the instance (type: FIXED|CUSTOM — for CUSTOM specify cpu,ram,disk etc.)
    #[command(about = "Resize an instance", long_about = "Change a plan; specify `--type FIXED` with `--product-id` or `--type CUSTOM` with specific resource values (cpu, ram-in-gb, disk-in-gb, bandwidth-in-tb).")]
    Resize { instance_id: String, #[arg(long)] r#type: String, #[arg(long)] product_id: Option<String>, #[arg(long)] cpu: Option<i64>, #[arg(long)] ram_in_gb: Option<i64>, #[arg(long)] disk_in_gb: Option<i64>, #[arg(long)] bandwidth_in_tb: Option<i64> },
    /// Add traffic amount (e.g., 50) to an instance
    #[command(about = "Add traffic to an instance", long_about = "Add additional traffic capacity to an instance using a numeric `--amount` (e.g., 50).")]
    AddTraffic { instance_id: String, amount: f64 },
    /// Trigger subscription refund (idempotent API query)
    #[command(about = "Request a subscription refund", long_about = "Trigger a subscription refund for an instance; results are returned as the API response and may contain success/failure codes.")]
    SubscriptionRefund { instance_id: String },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // CLI parsing
    let cli = Cli::parse();

    // If CLI provided an env-file or not, we will load it per command below
    // Note: we avoid constructing a default `state` here; commands build the per-command state
    // using `build_state_from_env` so we can pass a custom `--env-file` when executing commands.

    // Dispatch CLI commands. If no command provided, serve the web app by default
    if cli.command.is_none() {
    let state = build_state_from_env(None).await;
    start_server(state, DEFAULT_HOST, DEFAULT_PORT).await;
        return;
    }
    match cli.command.unwrap() {
        Commands::Serve {
            host,
            port,
            env_file,
        } => {
            let state = build_state_from_env(env_file.as_deref()).await;
            start_server(state, &host, port).await;
            return;
        }
        Commands::CheckConfig { env_file } => {
            let state = build_state_from_env(env_file.as_deref()).await;
            // Basic check: ensure API base and token exist; optionally ping regions
            let mut ok = true;
            if state.api_base_url.trim().is_empty() {
                eprintln!("API_BASE_URL is not configured");
                ok = false;
            }
            if state.api_token.trim().is_empty() {
                eprintln!("API_TOKEN is not configured");
                ok = false;
            }
            if !ok {
                process::exit(1);
            }
            let resp = api_call_wrapper(&state, "GET", "/v1/regions", None, None).await;
            if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
                println!("Configuration looks valid (regions returned)");
                process::exit(0);
            } else {
                eprintln!(
                    "Configuration appears invalid: {}",
                    serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into())
                );
                process::exit(1);
            }
        }
        Commands::Users { sub } => {
            let state = build_state_from_env(None).await;
            match sub {
                UserCommands::List => {
                    let users = state.users.lock().unwrap();
                    println!("username\trole\tassigned_instances");
                    for (u, rec) in users.iter() {
                        let assigned = if rec.assigned_instances.is_empty() {
                            String::new()
                        } else {
                            rec.assigned_instances.join(", ")
                        };
                        println!("{}\t{}\t{}", u, rec.role, assigned);
                    }
                    return;
                }
                UserCommands::Add {
                    username,
                    password,
                    role,
                } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    if users.contains_key(&uname) {
                        eprintln!("User '{}' already exists", uname);
                        process::exit(1);
                    }
                    let hash = generate_password_hash(&password);
                    users.insert(
                        uname.clone(),
                        UserRecord {
                            password: hash,
                            role: role.clone(),
                            assigned_instances: vec![],
                        },
                    );
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("Failed to persist users.json: {}", e);
                        process::exit(1);
                    }
                    println!("User '{}' added", uname);
                    return;
                }
                UserCommands::ResetPassword { username, password } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    if let Some(rec) = users.get_mut(&uname) {
                        rec.password = generate_password_hash(&password);
                    } else {
                        eprintln!("User '{}' not found", uname);
                        process::exit(1);
                    }
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("Failed to persist users.json: {}", e);
                        process::exit(1);
                    }
                    println!("Password for '{}' updated", uname);
                    return;
                }
                UserCommands::AddOwner {
                    username,
                    password,
                    force,
                } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    // If an owner already exists and we're not forcing, error out
                    let owner_exists = users.values().any(|r| r.role == "owner");
                    if owner_exists && !force {
                        eprintln!(
                            "An owner user already exists; use --force to create another owner or overwrite"
                        );
                        process::exit(1);
                    }
                    // If the username exists and force is not set, fail (consistent with `Add` semantics)
                    if users.contains_key(&uname) && !force {
                        eprintln!("User '{}' already exists; use --force to overwrite", uname);
                        process::exit(1);
                    }
                    let hash = generate_password_hash(&password);
                    users.insert(
                        uname.clone(),
                        UserRecord {
                            password: hash,
                            role: "owner".to_string(),
                            assigned_instances: vec![],
                        },
                    );
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("Failed to persist users.json: {}", e);
                        process::exit(1);
                    }
                    println!("Owner '{}' created", uname);
                    return;
                }
            }
        }
        Commands::Instances { sub } => {
            let state = build_state_from_env(None).await;
            match sub {
                InstanceCommands::List { username } => {
                    let uname = username.unwrap_or_default();
                    let list = load_instances_for_user_wrapper(&state, &uname).await;
                    println!("id\thostname\tstatus");
                    for i in list {
                        println!("{}\t{}\t{}", i.id, i.hostname, i.status);
                    }
                    return;
                }
                InstanceCommands::Show { instance_id } => {
                    let endpoint = format!("/v1/instances/{}", instance_id);
                    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::PowerOn { instance_id } => {
                    let payload = simple_instance_action(&state, "poweron", &instance_id).await;
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::PowerOff { instance_id } => {
                    let payload = simple_instance_action(&state, "poweroff", &instance_id).await;
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::Reset { instance_id } => {
                    let payload = simple_instance_action(&state, "reset", &instance_id).await;
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::Delete { instance_id } => {
                    let endpoint = format!("/v1/instances/{}", instance_id);
                    let payload = api_call_wrapper(&state, "DELETE", &endpoint, None, None).await;
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::ChangePass { instance_id } => {
                    let endpoint = format!("/v1/instances/{}/change-pass", instance_id);
                    let payload = api_call_wrapper(&state, "POST", &endpoint, None, None).await;
                    if let Some(pass) = payload.get("data").and_then(|d| d.get("password")).and_then(|v| v.as_str()) {
                        println!("New password for {}: {}", instance_id, pass);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "<non-json>".into()));
                    }
                    return;
                }
                InstanceCommands::ChangeOs { instance_id, os_id } => {
                    let endpoint = format!("/v1/instances/{}/change-os", instance_id);
                    let payload = serde_json::json!({"osId": os_id});
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::Resize { instance_id, r#type, product_id, cpu, ram_in_gb, disk_in_gb, bandwidth_in_tb } => {
                    let endpoint = format!("/v1/instances/{}/resize", instance_id);
                    let mut payload = serde_json::json!({"type": r#type});
                    if payload.get("type").and_then(|t| t.as_str()).unwrap_or("") == "FIXED" {
                        if let Some(pid) = product_id {
                            payload["productId"] = serde_json::Value::from(pid);
                        }
                    } else {
                        let mut obj = serde_json::Map::new();
                        if let Some(cpu) = cpu { obj.insert("cpu".into(), serde_json::Value::from(cpu)); }
                        if let Some(ram) = ram_in_gb { obj.insert("ramInGB".into(), serde_json::Value::from(ram)); }
                        if let Some(disk) = disk_in_gb { obj.insert("diskInGB".into(), serde_json::Value::from(disk)); }
                        if let Some(bw) = bandwidth_in_tb { obj.insert("bandwidthInTB".into(), serde_json::Value::from(bw)); }
                        if !obj.is_empty() { payload["resource"] = serde_json::Value::Object(obj); }
                    }
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::AddTraffic { instance_id, amount } => {
                    let endpoint = format!("/v1/instances/{}/add-traffic", instance_id);
                    let payload = serde_json::json!({"amount": amount});
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
                InstanceCommands::SubscriptionRefund { instance_id } => {
                    let endpoint = format!("/v1/instances/{}/subscription-refund", instance_id);
                    let resp = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into()));
                    return;
                }
            }
        }
    }

}
