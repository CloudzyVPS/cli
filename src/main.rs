use askama::Template;
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use urlencoding::encode;
use std::process;
use hex::encode as hex_encode;
use rand::rngs::OsRng;
use rand::RngCore;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use axum_extra::extract::cookie::{CookieJar, Cookie};
use std::env;
use std::path::Path;
// No-op logging ignore endpoint list for request logging (customize as necessary)
// API endpoints we should not log (avoid leaking full OS/Products payloads)
// Also ignore these common web routes to avoid verbose logging
static LOGGING_IGNORE_ENDPOINTS: &[&str] = &["/v1/os", "/v1/products", "/os", "/products"];
// Simple helper to return a plaintext HTML response
fn plain_html<S: AsRef<str>>(s: S) -> Response {
    Html(format!("<!DOCTYPE html><html><body><p>{}</p></body></html>", s.as_ref())).into_response()
}

// Simple user record persisted to users.json
#[derive(Clone, Serialize, Deserialize)]
struct UserRecord {
    password: String,
    role: String,
    assigned_instances: Vec<String>,
}

// AppState used across handlers
#[derive(Clone)]
struct AppState {
    users: Arc<Mutex<HashMap<String, UserRecord>>>,
    sessions: Arc<Mutex<HashMap<String, String>>>,
    flash_store: Arc<Mutex<HashMap<String, Vec<String>>>>,
    default_customer_cache: Arc<Mutex<Option<String>>>,
    api_base_url: String,
    api_token: String,
    public_base_url: String,
    client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}
impl OneOrMany {
    fn to_csv(self) -> String {
        match self {
            OneOrMany::One(s) => s,
            OneOrMany::Many(v) => v.join(","),
        }
    }
}

fn persist_users_file(users_arc: &Arc<Mutex<HashMap<String, UserRecord>>>) -> Result<(), std::io::Error> {
    let users = users_arc.lock().unwrap();
    let mut serialized: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (u, rec) in users.iter() {
        serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
    }
    std::fs::write("users.json", serde_json::to_string_pretty(&serde_json::Value::Object(serialized))?)
}

fn parse_urlencoded_body(body: &axum::body::Bytes) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let raw = String::from_utf8_lossy(body);
    for pair in raw.split('&') {
        if pair.is_empty() { continue; }
        let mut parts = pair.splitn(2, '=');
        let key_enc = parts.next().unwrap_or("");
        let val_enc = parts.next().unwrap_or("");
        let key = urlencoding::decode(key_enc).unwrap_or_else(|_| key_enc.into()).to_string();
        let val = urlencoding::decode(val_enc).unwrap_or_else(|_| val_enc.into()).to_string();
        map.entry(key).or_default().push(val);
    }
    map
}

// PBKDF2 iterations constant
const PBKDF2_ITERATIONS: u32 = 100_000;

// Generate a werkzeug-compatible PBKDF2 password string
fn generate_password_hash(password: &str) -> String {
    let mut salt_bytes = [0u8; 12];
    // try to use OsRng; if not available, fallback to rand
    rand::rngs::OsRng.fill_bytes(&mut salt_bytes);
    let salt = hex::encode(salt_bytes);
    let mut dk = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt.as_bytes(), PBKDF2_ITERATIONS, &mut dk);
    let hash_hex = hex::encode(dk);
    format!("pbkdf2:sha256:{}${}${}", PBKDF2_ITERATIONS, salt, hash_hex)
}

// Verify a PBKDF2 password string against a candidate
fn verify_password(stored: &str, candidate: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("pbkdf2:sha256:") {
        if let Some((iter_s, salt_hash)) = rest.split_once('$') {
            if let Some((salt, expected_hash)) = salt_hash.split_once('$') {
                if let Ok(iter) = iter_s.parse::<u32>() {
                    let mut dk = [0u8; 32];
                    pbkdf2_hmac::<Sha256>(candidate.as_bytes(), salt.as_bytes(), iter, &mut dk);
                    let computed = hex::encode(dk);
                    return computed == expected_hash;
                }
            }
        }
    }
    false
}

// Generate a random session ID (hex)
fn random_session_id() -> String {
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

// Build an AppState instance from env and users.json
fn build_state_from_env(env_file: Option<&str>) -> AppState {
    // load optional env file
    if let Some(path) = env_file {
        dotenvy::from_path(Path::new(path)).ok();
    } else {
        dotenvy::dotenv().ok();
    }
    let api_base_url = env::var("API_BASE_URL").unwrap_or_default();
    let api_token = env::var("API_TOKEN").unwrap_or_default();
    let public_base_url = sanitize_base_url(&env::var("PUBLIC_BASE_URL").unwrap_or_default());
    let client = reqwest::Client::new();

    let path = Path::new("users.json");
    let mut map: HashMap<String, UserRecord> = HashMap::new();
    if path.exists() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = json_val.as_object() {
                    for (k, v) in obj.iter() {
                        if let Some(pw) = v.get("password").and_then(|x| x.as_str()) {
                            let role = v
                                .get("role")
                                .and_then(|x| x.as_str())
                                .unwrap_or("admin")
                                .to_string();
                            let assigned_instances = v
                                .get("assigned_instances")
                                .and_then(|a| a.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_else(|| vec![]);
                            map.insert(
                                k.to_lowercase(),
                                UserRecord {
                                    password: pw.to_string(),
                                    role,
                                    assigned_instances,
                                },
                            );
                        }
                    }
                }
            }
        }
    } else {
        // create default owner
        let salt = {
            let mut b = [0u8; 12];
            rand::rngs::OsRng.fill_bytes(&mut b);
            hex::encode(b)
        };
        let mut dk = [0u8; 32];
        pbkdf2_hmac::<Sha256>(b"owner123", salt.as_bytes(), PBKDF2_ITERATIONS, &mut dk);
        let hash_hex = hex::encode(dk);
        let full = format!("pbkdf2:sha256:{}${}${}", PBKDF2_ITERATIONS, salt, hash_hex);
        map.insert(
            "owner".into(),
            UserRecord {
                password: full,
                role: "owner".into(),
                assigned_instances: vec![],
            },
        );
        let mut serialized: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for (u, rec) in map.iter() {
            serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
        }
        let _ = std::fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::Value::Object(serialized)).unwrap(),
        );
    }

    AppState {
        users: Arc::new(Mutex::new(map)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        flash_store: Arc::new(Mutex::new(HashMap::new())),
        default_customer_cache: Arc::new(Mutex::new(None)),
        api_base_url,
        api_token,
        public_base_url,
        client,
    }
}

// Global template context injected into most page templates
// (already implemented via build_template_globals/TemplateGlobals)
#[derive(Template)]
#[template(path = "regions.html")]
struct RegionsPageTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    regions: &'a [Region],
}

#[derive(Template)]
#[template(path = "products.html")]
struct ProductsPageTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    regions: &'a [Region],
    selected_region: Option<&'a Region>,
    active_region_id: String,
    requested_region: Option<String>,
    products: &'a [ProductView],
}

#[derive(Template)]
#[template(path = "os.html")]
struct OsCatalogTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    os_list: &'a [OsItem],
}

#[derive(Template)]
#[template(path = "applications.html")]
struct ApplicationsTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    apps: &'a [ApplicationView],
}

#[derive(Template)]
#[template(path = "instance_detail.html")]
struct InstanceDetailTemplate {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    instance_id: String,
    hostname: String,
    details: Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "bulk_refund.html")]
struct BulkRefundTemplate {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
}

fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_get))
        .route("/login", get(login_get).post(login_post))
        .route("/logout", post(logout_post))
        .route("/users", get(users_list).post(users_create))
        .route("/users/:username/reset-password", post(reset_password))
        .route("/users/:username/role", post(update_role))
        .route("/users/:username/delete", post(delete_user))
        .route("/access", get(access_get))
        .route("/access/:username", post(update_access))
        .route("/ssh-keys", get(ssh_keys_get).post(ssh_keys_post))
        .route("/instances", get(instances_real))
        .route("/regions", get(regions_get))
        .route("/products", get(products_get))
        .route("/os", get(os_get))
        .route("/applications", get(applications_get))
        .route("/create/step-1", get(create_step_1))
        .route("/create/step-2", get(create_step_2))
        .route("/create/step-3", get(create_step_3))
        .route("/create/step-4", get(create_step_4))
        .route("/create/step-5", get(create_step_5))
        .route("/create/step-6", get(create_step_6))
        .route(
            "/create/step-7",
            get(create_step_7_get).post(create_step_7_post),
        )
        .route("/create/step-8", get(create_step_8))
        .route("/instance/:instance_id", get(instance_detail))
        .route("/instance/:instance_id/poweron", get(instance_poweron))
        .route("/instance/:instance_id/poweroff", get(instance_poweroff))
        .route("/instance/:instance_id/reset", get(instance_reset))
        .route(
            "/instance/:instance_id/change-pass",
            get(instance_change_pass),
        )
        .route("/instance/:instance_id/change-os", get(instance_change_os))
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
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

async fn start_server(state: AppState, host: &str, port: u16) {
    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    let app = build_app(state.clone());
    tracing::info!(%addr, "Starting Zyffiliate Rust server");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

// Individual route handlers (stubs). Later these will load data & real templates.
#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    error: Option<String>,
}

async fn login_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(_username) = current_username_from_jar(&state, &jar) {
        // If already logged in, redirect to `/` which will then send the
        // user to the correct default landing (instances or create).
        return Redirect::to("/").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        LoginTemplate {
            current_user,
            api_hostname,
            base_url: base_url.clone(),
            flash_messages,
            has_flash_messages,
            error: None,
        }
        .render()
        .unwrap(),
    )
}

async fn login_post(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let uname = form.username.trim().to_lowercase();
    let users = state.users.lock().unwrap();
    if let Some(record) = users.get(&uname) {
        if verify_password(&record.password, &form.password) {
            drop(users);
            let sid = random_session_id();
            state
                .sessions
                .lock()
                .unwrap()
                .insert(sid.clone(), uname.clone());
            let mut cookie = Cookie::new("session_id", sid);
            cookie.set_path("/");
            cookie.set_http_only(true);
            let target = resolve_default_endpoint(&state, &uname);
            return (jar.add(cookie), Redirect::to(&target)).into_response();
        }
    }
    drop(users);
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        LoginTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            error: Some("Invalid credentials".into()),
        }
        .render()
        .unwrap(),
    )
}
async fn logout_post(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(sid) = jar.get("session_id").map(|c| c.value().to_string()) {
        state.sessions.lock().unwrap().remove(&sid);
    }
    let cleared = jar.remove(Cookie::new("session_id", ""));
    Redirect::to("/login").into_response_with(cleared)
}

trait IntoResponseWithJar {
    fn into_response_with(self, jar: CookieJar) -> axum::response::Response;
}

impl IntoResponseWithJar for Redirect {
    fn into_response_with(self, jar: CookieJar) -> axum::response::Response {
        (jar, self).into_response()
    }
}

fn current_username_from_jar(state: &AppState, jar: &CookieJar) -> Option<String> {
    let sid = session_id_from_jar(jar)?;
    state.sessions.lock().unwrap().get(&sid).cloned()
}

fn session_id_from_jar(jar: &CookieJar) -> Option<String> {
    jar.get("session_id").map(|c| c.value().to_string())
}

fn take_flash_messages(state: &AppState, jar: &CookieJar) -> Vec<String> {
    let Some(session_id) = session_id_from_jar(jar) else {
        return Vec::new();
    };
    state
        .flash_store
        .lock()
        .unwrap()
        .remove(&session_id)
        .unwrap_or_default()
}

fn resolve_default_endpoint(state: &AppState, username: &str) -> String {
    let users = state.users.lock().unwrap();
    if let Some(user) = users.get(username) {
        if user.role == "owner" {
            return "/create/step-1".to_string();
        }
        return "/instances".to_string();
    }
    "/login".to_string()
}

fn sanitize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "http://localhost:5000".to_string()
    } else {
        trimmed.to_string()
    }
}

fn absolute_url(state: &AppState, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let mut base = state.public_base_url.clone();
    if !path.starts_with('/') {
        base.push('/');
        base.push_str(path);
        return base;
    }
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return base;
    }
    format!("{}/{}", base, trimmed)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CurrentUser {
    username: String,
    role: String,
}

fn build_current_user(state: &AppState, jar: &CookieJar) -> Option<CurrentUser> {
    let uname = current_username_from_jar(state, jar)?;
    let users = state.users.lock().unwrap();
    let rec = users.get(&uname)?;
    Some(CurrentUser {
        username: uname,
        role: rec.role.clone(),
    })
}

#[derive(Default)]
struct TemplateGlobals {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
}

fn build_template_globals(state: &AppState, jar: &CookieJar) -> TemplateGlobals {
    let flash_messages = take_flash_messages(state, jar);
    TemplateGlobals {
        current_user: build_current_user(state, jar),
        api_hostname: hostname_from_url(&state.api_base_url),
        base_url: state.public_base_url.clone(),
        has_flash_messages: !flash_messages.is_empty(),
        flash_messages,
    }
}

fn hostname_from_url(u: &str) -> String {
    let s = u.trim();
    if s.is_empty() {
        return "".into();
    }
    // Remove scheme if present
    let s = if let Some(idx) = s.find("://") { &s[idx+3..] } else { s };
    // Take up to first slash
    let host = s.split('/').next().unwrap_or(s);
    host.to_string()
}

fn inject_context(state: &AppState, jar: &CookieJar, mut html: String) -> Response {
    let current = build_current_user(state, jar);
    let api_hostname = hostname_from_url(&state.api_base_url);
    // Insert a hidden context div right after opening <body>
    let ctx_div = format!("<div id='ctx' data-api-hostname='{}' data-base-url='{}' data-current-username='{}' data-current-role='{}' style='display:none'></div>",
                          api_hostname,
                          state.public_base_url,
                          current.as_ref().map(|c| c.username.clone()).unwrap_or_default(),
                          current.as_ref().map(|c| c.role.clone()).unwrap_or_default());
    if let Some(pos) = html.find("<body>") {
        let insert_pos = pos + "<body>".len();
        html.insert_str(insert_pos, &ctx_div);
    } else {
        html.push_str(&ctx_div);
    }
    Html(html).into_response()
}

// ---------- Helper Parsing Functions (Wizard) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaseState {
    hostnames: Vec<String>,
    region: String,
    instance_class: String,
    plan_type: String,
    assign_ipv4: bool,
    assign_ipv6: bool,
    floating_ip_count: i32,
    ssh_key_ids: Vec<i64>,
    os_id: String,
}

fn parse_flag(value: Option<&String>, default: bool) -> bool {
    match value {
        Some(v) => {
            let t = v.trim().to_lowercase();
            if t.is_empty() {
                default
            } else {
                matches!(t.as_str(), "1" | "true" | "yes" | "on")
            }
        }
        None => default,
    }
}

fn parse_optional_int(value: Option<&String>) -> Option<i32> {
    value.and_then(|v| {
        let t = v.trim();
        if t.is_empty() {
            None
        } else {
            t.parse::<i32>().ok()
        }
    })
}

fn parse_int_list(values: &[String]) -> Vec<i64> {
    values
        .iter()
        .filter_map(|v| {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<i64>().ok()
            }
        })
        .collect()
}

fn parse_wizard_base(query: &HashMap<String, String>) -> BaseState {
    let mut hostnames: Vec<String> = query
        .get("hostnames")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Additionally capture repeated 'hostnames' occurrences if any (Axum Query collapses duplicates; keep simple)
    hostnames.retain(|h| !h.is_empty());
    let region = query
        .get("region")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let instance_class = query
        .get("instance_class")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".into());
    let plan_type = query
        .get("plan_type")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| matches!(s.as_str(), "fixed" | "custom"))
        .unwrap_or_else(|| "fixed".into());
    let assign_ipv4 = parse_flag(query.get("assign_ipv4"), true);
    let assign_ipv6 = parse_flag(query.get("assign_ipv6"), false);
    let floating_ip_count = parse_optional_int(query.get("floating_ip_count")).unwrap_or(0);
    // ssh_key_ids may appear as comma separated
    let ssh_raw = query
        .get("ssh_key_ids")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ssh_key_ids = parse_int_list(&ssh_raw);
    let os_id = query
        .get("os_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    BaseState {
        hostnames,
        region,
        instance_class,
        plan_type,
        assign_ipv4,
        assign_ipv6,
        floating_ip_count,
        ssh_key_ids,
        os_id,
    }
}

fn build_base_query_pairs(state: &BaseState) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for h in &state.hostnames {
        pairs.push(("hostnames".into(), h.clone()));
    }
    if !state.region.is_empty() {
        pairs.push(("region".into(), state.region.clone()));
    }
    pairs.push(("instance_class".into(), state.instance_class.clone()));
    pairs.push(("plan_type".into(), state.plan_type.clone()));
    pairs.push(("assign_ipv4".into(), (state.assign_ipv4 as u8).to_string()));
    pairs.push(("assign_ipv6".into(), (state.assign_ipv6 as u8).to_string()));
    if state.floating_ip_count > 0 {
        pairs.push((
            "floating_ip_count".into(),
            state.floating_ip_count.to_string(),
        ));
    }
    for id in &state.ssh_key_ids {
        pairs.push(("ssh_key_ids".into(), id.to_string()));
    }
    if !state.os_id.is_empty() {
        pairs.push(("os_id".into(), state.os_id.clone()));
    }
    pairs
}

fn build_query_string(pairs: &[(String, String)]) -> String {
    let mut first = true;
    let mut out = String::new();
    for (k, v) in pairs {
        if !first {
            out.push('&');
        } else {
            first = false;
        }
        out.push_str(&encode(k));
        out.push('=');
        out.push_str(&encode(v));
    }
    out
}

// ---------- Regions Loader ----------
#[derive(Serialize, Deserialize, Clone)]
struct Region {
    id: String,
    name: String,
    abbr: Option<String>,
    description: Option<String>,
    is_active: bool,
    is_premium: bool,
    tags: Option<String>,
    ram_threshold_gb: Option<f64>,
    disk_threshold_gb: Option<f64>,
    config: Value,
}

async fn load_regions(state: &AppState) -> (Vec<Region>, HashMap<String, Region>) {
    let payload = api_call(state, "GET", "/v1/regions", None, None).await;
    let mut regions = Vec::new();
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for r in arr {
                if let Some(obj) = r.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string();
                    let is_active = obj
                        .get("isActive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let config = obj.get("config").cloned().unwrap_or(Value::Null);
                    let ram_threshold_gb = config.get("ramThresholdInGB").and_then(|v| v.as_f64());
                    let disk_threshold_gb =
                        config.get("diskThresholdInGB").and_then(|v| v.as_f64());
                    let abbr = obj
                        .get("abbr")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let description = obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let is_premium = obj
                        .get("isPremium")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let tags = obj
                        .get("tags")
                        .map(|t| {
                            if let Some(arr) = t.as_array() {
                                arr.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            } else {
                                t.as_str().unwrap_or("").to_string()
                            }
                        })
                        .filter(|s| !s.is_empty());
                    if is_active {
                        regions.push(Region {
                            id,
                            name,
                            abbr,
                            description,
                            is_active,
                            is_premium,
                            tags,
                            ram_threshold_gb,
                            disk_threshold_gb,
                            config,
                        });
                    }
                }
            }
        }
    }
    let lookup = regions.iter().cloned().map(|r| (r.id.clone(), r)).collect();
    (regions, lookup)
}

// ---------- Wizard Step 1 Template ----------
#[derive(Clone, Default)]
struct Step1FormData {
    region: String,
    instance_class: String,
    plan_type: String,
}

#[derive(Template)]
#[template(path = "create/start.html")]
struct Step1Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    regions: &'a [Region],
    form_data: Step1FormData,
}

async fn create_step_1(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    let (regions, _lookup) = load_regions(&state).await;
    let mut region_sel = base.region.clone();
    if region_sel.is_empty() && !regions.is_empty() {
        region_sel = regions[0].id.clone();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_data = Step1FormData {
        region: region_sel,
        instance_class: base.instance_class.clone(),
        plan_type: base.plan_type.clone(),
    };
    inject_context(
        &state,
        &jar,
        Step1Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            regions: &regions,
            form_data,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 2 (Hostnames & IP Assignment) ----------
#[derive(Clone)]
struct Step2FormData {
    hostnames_text: String,
    assign_ipv4: bool,
    assign_ipv6: bool,
    floating_ip_count: String,
}

#[derive(Template)]
#[template(path = "create/hostnames.html")]
struct Step2Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    form_data: Step2FormData,
    back_url: String,
    submit_url: String,
}

async fn create_step_2(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let mut base = parse_wizard_base(&q);
    if base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    // If hostnames passed as comma separated in textarea update parsing
    if let Some(raw_hosts) = q.get("hostnames") {
        base.hostnames = raw_hosts
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, "/create/step-1")
    } else {
        absolute_url(&state, &format!("/create/step-1?{}", back_q))
    };
    let hostnames_text = base.hostnames.join(", ");
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_data = Step2FormData {
        hostnames_text,
        assign_ipv4: base.assign_ipv4,
        assign_ipv6: base.assign_ipv6,
        floating_ip_count: base.floating_ip_count.to_string(),
    };
    inject_context(
        &state,
        &jar,
        Step2Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            form_data,
            back_url,
            submit_url: absolute_url(&state, "/create/step-3"),
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 3 (Product selection or custom resources) ----------
#[derive(Serialize, Deserialize, Clone)]
struct ProductEntry {
    term: String,
    value: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ProductView {
    id: String,
    name: String,
    display_name: String,
    description: String,
    tags: String,
    spec_entries: Vec<ProductEntry>,
    price_entries: Vec<ProductEntry>,
}

/// Build a user-friendly display name for a product from its specs.
/// Uses CPU, RAM, Disk if available; falls back to name/tags/price or "Plan".
fn build_product_display_name(
    region: &str,
    id: &str,
    name: &str,
    tags: &str,
    spec_entries: &[ProductEntry],
    price_entries: &[ProductEntry],
) -> String {
    // Try to extract CPU, RAM, Disk from spec_entries
    let cpu = spec_entries
        .iter()
        .find(|e| {
            let t = e.term.to_lowercase();
            t.contains("cpu") || t.contains("vcpu") || t.contains("core")
        })
        .map(|e| e.value.clone());
    let ram = spec_entries
        .iter()
        .find(|e| {
            let t = e.term.to_lowercase();
            t.contains("ram") || t.contains("memory")
        })
        .map(|e| e.value.clone());
    let disk = spec_entries
        .iter()
        .find(|e| {
            let t = e.term.to_lowercase();
            t.contains("disk") || t.contains("storage") || t.contains("ssd") || t.contains("nvme")
        })
        .map(|e| e.value.clone());

    // Build display name from specs if we have at least CPU or RAM
    if cpu.is_some() || ram.is_some() {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = cpu.as_ref() {
            parts.push(c.clone());
        }
        if let Some(r) = ram.as_ref() {
            parts.push(r.clone());
        }
        if let Some(d) = disk.as_ref() {
            parts.push(d.clone());
        }
        if !parts.is_empty() {
            return parts.join(" · ");
        }
    }

    // Fallback: use name if it's different from id and not empty
    if !name.is_empty() && name != id {
        return name.to_string();
    }

    // Fallback: use tags if not empty
    if !tags.is_empty() {
        return tags.to_string();
    }

    // Fallback: use the first price
    if let Some(entry) = price_entries.first() {
        if !entry.value.is_empty() {
            return entry.value.clone();
        }
    }

    // Build a fallback that includes region and main resource counts when possible
    let mut parts: Vec<String> = Vec::new();
    if !region.trim().is_empty() {
        parts.push(region.to_string());
    }
    if let Some(v) = cpu.as_ref() {
        // Avoid adding units if the spec value already contains text like "vCPU".
        if v.to_lowercase().contains("vcpu") || v.to_lowercase().contains("cpu") {
            parts.push(v.clone());
        } else {
            parts.push(format!("{} vCPU", v));
        }
    }
    if let Some(v) = ram.as_ref() {
        // Keep unit if included already
        if v.to_lowercase().contains("gb") || v.to_lowercase().contains("ram") {
            parts.push(v.clone());
        } else {
            parts.push(format!("{} RAM", v));
        }
    }
    if let Some(v) = disk.as_ref() {
        if v.to_lowercase().contains("gb") || v.to_lowercase().contains("disk") || v.to_lowercase().contains("ssd") {
            parts.push(v.clone());
        } else {
            parts.push(format!("{} Disk", v));
        }
    }
    if !parts.is_empty() {
        return parts.join(" · ");
    }
    // Final fallback
    "Plan".to_string()
}

#[derive(Template)]
#[template(path = "create/fixed.html")]
struct Step3FixedTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    products: &'a [ProductView],
    has_products: bool,
    selected_product_id: String,
    region_name: String,
    floating_ip_count: String,
    back_url: String,
    submit_url: String,
    restart_url: String,
    ssh_key_ids_csv: String,
    hostnames_csv: String,
}

#[derive(Clone)]
struct CustomPlanFormValues {
    cpu: String,
    ram_in_gb: String,
    disk_in_gb: String,
    bandwidth_in_tb: String,
}

#[derive(Template)]
#[template(path = "create/custom.html")]
struct Step3CustomTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    region_name: String,
    floating_ip_count: String,
    back_url: String,
    submit_url: String,
    requirements: Vec<String>,
    minimum_ram: i32,
    minimum_disk: i32,
    form_values: CustomPlanFormValues,
    ssh_key_ids_csv: String,
    hostnames_csv: String,
}

fn value_to_short_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => arr
            .iter()
            .map(value_to_short_string)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(obj) => {
            let mut parts = Vec::new();
            for (key, val) in obj {
                parts.push(format!("{}: {}", key, value_to_short_string(val)));
            }
            parts.join(", ")
        }
        Value::Null => String::new(),
    }
}

fn collect_product_entries(value: Option<&Value>) -> Vec<ProductEntry> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                if let Some(obj) = item.as_object() {
                    let term = obj
                        .get("term")
                        .or_else(|| obj.get("label"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Detail")
                        .to_string();
                    let val = obj
                        .get("value")
                        .or_else(|| obj.get("display"))
                        .map(value_to_short_string)
                        .unwrap_or_else(|| value_to_short_string(item));
                    Some(ProductEntry { term, value: val })
                } else if !item.is_null() {
                    Some(ProductEntry {
                        term: "Detail".into(),
                        value: value_to_short_string(item),
                    })
                } else {
                    None
                }
            })
            .collect(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(k, v)| ProductEntry {
                term: k.clone(),
                value: value_to_short_string(v),
            })
            .collect(),
        Some(other) if !other.is_null() => vec![ProductEntry {
            term: "Detail".into(),
            value: value_to_short_string(other),
        }],
        _ => Vec::new(),
    }
}

fn tags_from_value(value: Option<&Value>) -> Option<String> {
    value.and_then(|val| {
        if let Some(arr) = val.as_array() {
            let joined = arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        } else if let Some(s) = val.as_str() {
            if s.trim().is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        } else {
            let text = value_to_short_string(val);
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
    })
}

async fn load_products(state: &AppState, region_id: &str) -> Vec<ProductView> {
    let params = vec![("regionId".into(), region_id.to_string())];
    let payload = api_call(state, "GET", "/v1/products", None, Some(params)).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .map(|n| n.to_string())
                        .or_else(|| {
                            obj.get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    let plan = obj.get("plan").and_then(|v| v.as_object());
                    let name = plan
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string();
                    let description = plan
                        .and_then(|p| p.get("description"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tags = tags_from_value(obj.get("tags")).unwrap_or_default();
                    let spec_entries = collect_product_entries(plan.and_then(|p| p.get("specs")));
                    let price_entries = collect_product_entries(
                        plan.and_then(|p| p.get("prices"))
                            .or_else(|| plan.and_then(|p| p.get("pricing"))),
                    );
                    let display_name = build_product_display_name(
                        &region_id,
                        &id,
                        &name,
                        &tags,
                        &spec_entries,
                        &price_entries,
                    );
                    out.push(ProductView {
                        id,
                        name,
                        display_name,
                        description,
                        tags,
                        spec_entries,
                        price_entries,
                    });
                }
            }
        }
    }
    out
}

async fn create_step_3(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, "/create/step-2")
    } else {
        absolute_url(&state, &format!("/create/step-2?{}", back_q))
    };
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let hostnames_csv = base.hostnames.join(",");

    if base.plan_type == "fixed" {
        let products = load_products(&state, &base.region).await;
        let selected_product_id = q.get("product_id").cloned().unwrap_or_default();
        let TemplateGlobals {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
        } = build_template_globals(&state, &jar);
        // Use the outer variables defined above
        return inject_context(
            &state,
            &jar,
            Step3FixedTemplate {
                current_user,
                api_hostname,
                base_url,
                flash_messages,
                has_flash_messages,
                base_state: &base,
                products: &products,
                has_products: !products.is_empty(),
                selected_product_id,
                region_name: base.region.clone(),
                floating_ip_count: base.floating_ip_count.to_string(),
                back_url,
                submit_url: absolute_url(&state, "/create/step-4"),
                restart_url: absolute_url(&state, "/create/step-1"),
                ssh_key_ids_csv: ssh_key_ids_csv.clone(),
                hostnames_csv: hostnames_csv.clone(),
            }
            .render()
            .unwrap(),
        );
    }
    let cpu = q.get("cpu").cloned().unwrap_or_else(|| "2".into());
    let ram = q.get("ramInGB").cloned().unwrap_or_else(|| "4".into());
    let disk = q.get("diskInGB").cloned().unwrap_or_else(|| "50".into());
    let bw = q
        .get("bandwidthInTB")
        .cloned()
        .unwrap_or_else(|| "1".into());
    
    let hostnames_csv = base.hostnames.join(",");
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let form_values = CustomPlanFormValues {
        cpu,
        ram_in_gb: ram,
        disk_in_gb: disk,
        bandwidth_in_tb: bw,
    };
    inject_context(
        &state,
        &jar,
        Step3CustomTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            region_name: base.region.clone(),
            floating_ip_count: base.floating_ip_count.to_string(),
            back_url,
            submit_url: absolute_url(&state, "/create/step-5"),
            requirements: Vec::new(),
            minimum_ram: 1,
            minimum_disk: 1,
            form_values,
            ssh_key_ids_csv: ssh_key_ids_csv.clone(),
            hostnames_csv: hostnames_csv,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 4 (Extras for fixed plans) ----------
#[derive(Clone)]
struct ExtrasFormValues {
    extra_disk: String,
    extra_bandwidth: String,
}

#[derive(Template)]
#[template(path = "create/extras.html")]
struct Step4Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    floating_ip_count: String,
    product_id: String,
    ssh_key_ids_csv: String,
    hostnames_csv: String,
    extras: ExtrasFormValues,
    back_url: String,
    submit_url: String,
}

async fn create_step_4(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let hostnames_csv = base.hostnames.join(",");
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, "/create/step-3")
    } else {
        absolute_url(&state, &format!("/create/step-3?{}", back_q))
    };
    if base.plan_type != "fixed" {
        let next_pairs = build_base_query_pairs(&base);
        let next_q = build_query_string(&next_pairs);
        let next_url = if next_q.is_empty() {
            "/create/step-5".to_string()
        } else {
            format!("/create/step-5?{}", next_q)
        };
        return Redirect::to(&next_url).into_response();
    }
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let extras = ExtrasFormValues {
        extra_disk: q.get("extra_disk").cloned().unwrap_or_else(|| "0".into()),
        extra_bandwidth: q
            .get("extra_bandwidth")
            .cloned()
            .unwrap_or_else(|| "0".into()),
    };
    inject_context(
        &state,
        &jar,
        Step4Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            product_id,
            ssh_key_ids_csv: ssh_key_ids_csv,
            hostnames_csv: hostnames_csv,
            extras,
            back_url,
            submit_url: absolute_url(&state, "/create/step-5"),
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 5 (OS selection) ----------
#[derive(Serialize, Deserialize, Clone)]
struct OsItem {
    id: String,
    name: String,
    family: String,
    arch: Option<String>,
    version: Option<String>,
    min_ram: Option<String>,
    disk: Option<String>,
    description: Option<String>,
    is_default: bool,
}

#[derive(Clone, Default)]
struct CustomPlanCarry {
    cpu: String,
    ram_in_gb: String,
    disk_in_gb: String,
    bandwidth_in_tb: String,
}

#[derive(Template)]
#[template(path = "create/os.html")]
struct Step5Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    os_list: &'a [OsItem],
    selected_os_id: String,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    custom_plan: CustomPlanCarry,
    floating_ip_count: String,
    back_url: String,
    submit_url: String,
    hostnames_csv: String,
    ssh_key_ids_csv: String,
}

async fn load_os_list(state: &AppState) -> Vec<OsItem> {
    let payload = api_call(state, "GET", "/v1/os", None, None).await;
    let mut out = vec![];
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload
            .get("data")
            .and_then(|d| d.get("os"))
            .and_then(|o| o.as_array())
        {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let family = obj
                        .get("family")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let arch = obj
                        .get("arch")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let version = obj
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let min_ram = obj
                        .get("minRam")
                        .map(value_to_short_string)
                        .filter(|s| !s.is_empty());
                    let disk = obj
                        .get("disk")
                        .map(value_to_short_string)
                        .filter(|s| !s.is_empty());
                    let description = obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let is_default = obj
                        .get("isDefault")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    out.push(OsItem {
                        id,
                        name,
                        family,
                        arch,
                        version,
                        min_ram,
                        disk,
                        description,
                        is_default,
                    });
                }
            }
        }
    }
    out
}

#[derive(Serialize, Deserialize, Clone)]
struct ApplicationView {
    id: String,
    name: String,
    description: String,
    category: Option<String>,
    price: Option<String>,
    tags: Option<String>,
    is_featured: bool,
}

async fn load_applications(state: &AppState) -> Vec<ApplicationView> {
    let payload = api_call(state, "GET", "/v1/applications", None, None).await;
    let mut apps = Vec::new();
    if payload.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
        if let Some(arr) = payload
            .get("data")
            .and_then(|d| d.get("applications"))
            .and_then(|a| a.as_array())
        {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .map(|n| n.to_string())
                        .or_else(|| {
                            obj.get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string();
                    let description = obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let category = obj
                        .get("category")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let price = obj
                        .get("price")
                        .map(value_to_short_string)
                        .or_else(|| obj.get("pricing").map(value_to_short_string))
                        .filter(|s| !s.is_empty());
                    let tags = tags_from_value(obj.get("tags"));
                    let is_featured = obj
                        .get("isFeatured")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    apps.push(ApplicationView {
                        id,
                        name,
                        description,
                        category,
                        price,
                        tags,
                        is_featured,
                    });
                }
            }
        }
    }
    apps
}

async fn create_step_5(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if base.plan_type == "fixed" && product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let extra_disk = q.get("extra_disk").cloned().unwrap_or_else(|| "0".into());
    let extra_bandwidth = q
        .get("extra_bandwidth")
        .cloned()
        .unwrap_or_else(|| "0".into());
    let custom_plan = CustomPlanCarry {
        cpu: q.get("cpu").cloned().unwrap_or_else(|| "2".into()),
        ram_in_gb: q.get("ramInGB").cloned().unwrap_or_else(|| "4".into()),
        disk_in_gb: q.get("diskInGB").cloned().unwrap_or_else(|| "50".into()),
        bandwidth_in_tb: q
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into()),
    };
    let os_list = load_os_list(&state).await;
    let mut selected_os_id = base.os_id.clone();
    if selected_os_id.is_empty() {
        selected_os_id = q.get("os_id").cloned().unwrap_or_default();
    }
    if selected_os_id.is_empty() {
        selected_os_id = os_list
            .iter()
            .find(|o| o.is_default)
            .map(|o| o.id.clone())
            .or_else(|| os_list.first().map(|o| o.id.clone()))
            .unwrap_or_default();
    }
    let mut back_pairs = build_base_query_pairs(&base);
    let back_target = if base.plan_type == "fixed" {
        if !product_id.is_empty() {
            back_pairs.push(("product_id".into(), product_id.clone()));
        }
        back_pairs.push(("extra_disk".into(), extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), extra_bandwidth.clone()));
        "/create/step-4"
    } else {
        back_pairs.push(("cpu".into(), custom_plan.cpu.clone()));
        back_pairs.push(("ramInGB".into(), custom_plan.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), custom_plan.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), custom_plan.bandwidth_in_tb.clone()));
        "/create/step-3"
    };
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, back_target)
    } else {
        absolute_url(&state, &format!("{}?{}", back_target, back_q))
    };
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    inject_context(
        &state,
        &jar,
        Step5Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            os_list: &os_list,
            selected_os_id,
            product_id,
            extra_disk,
            extra_bandwidth,
            custom_plan,
            floating_ip_count: base.floating_ip_count.to_string(),
            back_url,
            submit_url: absolute_url(&state, "/create/step-6"),
            hostnames_csv: hostnames_csv,
            ssh_key_ids_csv: ssh_key_ids_csv,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 6 (SSH key selection) ----------
struct SelectableSshKey {
    id: String,
    name: String,
    selected: bool,
}

#[derive(Template)]
#[template(path = "create/ssh_keys.html")]
struct Step6Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    floating_ip_count: String,
    ssh_keys: Vec<SelectableSshKey>,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    custom_plan: CustomPlanCarry,
    back_url: String,
    submit_url: String,
    manage_keys_url: String,
    hostnames_csv: String,
}

async fn create_step_6(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let base = parse_wizard_base(&q);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if base.plan_type == "fixed" && product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let extra_disk = q.get("extra_disk").cloned().unwrap_or_else(|| "0".into());
    let extra_bandwidth = q
        .get("extra_bandwidth")
        .cloned()
        .unwrap_or_else(|| "0".into());
    let custom_plan = CustomPlanCarry {
        cpu: q.get("cpu").cloned().unwrap_or_else(|| "2".into()),
        ram_in_gb: q.get("ramInGB").cloned().unwrap_or_else(|| "4".into()),
        disk_in_gb: q.get("diskInGB").cloned().unwrap_or_else(|| "50".into()),
        bandwidth_in_tb: q
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into()),
    };
    let mut back_pairs = build_base_query_pairs(&base);
    let back_target = if base.plan_type == "fixed" {
        if !product_id.is_empty() {
            back_pairs.push(("product_id".into(), product_id.clone()));
        }
        back_pairs.push(("extra_disk".into(), extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), extra_bandwidth.clone()));
        "/create/step-5"
    } else {
        back_pairs.push(("cpu".into(), custom_plan.cpu.clone()));
        back_pairs.push(("ramInGB".into(), custom_plan.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), custom_plan.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), custom_plan.bandwidth_in_tb.clone()));
        "/create/step-5"
    };
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, back_target)
    } else {
        absolute_url(&state, &format!("{}?{}", back_target, back_q))
    };
    let customer_id = fetch_default_customer_id(&state).await;
    let ssh_keys = load_ssh_keys_api(&state, customer_id).await;
    let selected_ids: HashSet<String> =
        base.ssh_key_ids.iter().map(|id| id.to_string()).collect();
    let selectable: Vec<SelectableSshKey> = ssh_keys
        .into_iter()
        .map(|key| {
            let is_selected = selected_ids.contains(&key.id);
            SelectableSshKey {
                id: key.id,
                name: key.name,
                selected: is_selected,
            }
        })
        .collect();
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = base.ssh_key_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    inject_context(
        &state,
        &jar,
        Step6Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            ssh_keys: selectable,
            product_id,
            extra_disk,
            extra_bandwidth,
            custom_plan,
            back_url,
            submit_url: absolute_url(&state, "/create/step-7"),
            manage_keys_url: absolute_url(&state, "/ssh-keys"),
            hostnames_csv,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 7 (Review & Create) ----------
#[derive(Clone, Default)]
struct PlanReviewState {
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    cpu: String,
    ram_in_gb: String,
    disk_in_gb: String,
    bandwidth_in_tb: String,
}

#[derive(Template)]
#[template(path = "create/review.html")]
struct Step7Template<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    base_state: &'a BaseState,
    floating_ip_count: String,
    plan_state: PlanReviewState,
    plan_type_label: String,
    region_name: String,
    hostnames_display: String,
    plan_summary: Vec<ProductEntry>,
    has_plan_summary: bool,
    price_entries: Vec<ProductEntry>,
    has_price_entries: bool,
    selected_os_label: String,
    ssh_keys_display: String,
    selected_product_tags: Option<String>,
    selected_product_description: Option<String>,
    footnote_text: String,
    has_footnote: bool,
    back_url: String,
    submit_url: String,
    ssh_key_ids_csv: String,
    selected_product_name: Option<String>,
    hostnames_csv: String,
}

#[derive(Template)]
#[template(path = "create/result.html")]
struct Step8Template {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    back_url: String,
    status_label: String,
    code: Option<String>,
    detail: Option<String>,
    errors: Vec<String>,
    raw_json: String,
}

async fn create_step_7_core(
    state: AppState,
    jar: CookieJar,
    method: axum::http::Method,
    query: HashMap<String, String>,
    form: HashMap<String, String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let source = if method == axum::http::Method::POST {
        &form
    } else {
        &query
    };
    let base = parse_wizard_base(source);
    if base.hostnames.is_empty() || base.region.is_empty() {
        return Redirect::to("/create/step-1").into_response();
    }
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
    }
    let mut plan_state = PlanReviewState::default();
    if base.plan_type == "fixed" {
        plan_state.product_id = source.get("product_id").cloned().unwrap_or_default();
        if plan_state.product_id.is_empty() {
            return Redirect::to("/create/step-3").into_response();
        }
        plan_state.extra_disk = source
            .get("extra_disk")
            .cloned()
            .unwrap_or_else(|| "0".into());
        plan_state.extra_bandwidth = source
            .get("extra_bandwidth")
            .cloned()
            .unwrap_or_else(|| "0".into());
    } else {
        plan_state.cpu = source.get("cpu").cloned().unwrap_or_else(|| "2".into());
        plan_state.ram_in_gb = source.get("ramInGB").cloned().unwrap_or_else(|| "4".into());
        plan_state.disk_in_gb = source
            .get("diskInGB")
            .cloned()
            .unwrap_or_else(|| "50".into());
        plan_state.bandwidth_in_tb = source
            .get("bandwidthInTB")
            .cloned()
            .unwrap_or_else(|| "1".into());
    }
    if method == axum::http::Method::POST {
        let mut payload = serde_json::json!({
            "hostnames": base.hostnames,
            "region": base.region,
            "class": base.instance_class,
            "assignIpv4": base.assign_ipv4,
            "assignIpv6": base.assign_ipv6,
            "osId": base.os_id,
        });
        if base.floating_ip_count > 0 {
            payload["floatingIPCount"] = Value::from(base.floating_ip_count);
        }
        if !base.ssh_key_ids.is_empty() {
            payload["sshKeyIds"] = Value::from(base.ssh_key_ids.clone());
        }
        if base.plan_type == "fixed" {
            payload["productId"] = Value::from(plan_state.product_id.clone());
            let mut extras = serde_json::Map::new();
            if let Some(d) = plan_state
                .extra_disk
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
            {
                extras.insert("diskInGB".into(), Value::from(d));
            }
            if let Some(b) = plan_state
                .extra_bandwidth
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
            {
                extras.insert("bandwidthInTB".into(), Value::from(b));
            }
            if !extras.is_empty() {
                payload["extraResource"] = Value::Object(extras);
            }
        } else {
            let mut extras = serde_json::Map::new();
            if let Some(cpu) = plan_state.cpu.trim().parse::<i64>().ok() {
                extras.insert("cpu".into(), Value::from(cpu));
            }
            if let Some(ram) = plan_state.ram_in_gb.trim().parse::<i64>().ok() {
                extras.insert("ramInGB".into(), Value::from(ram));
            }
            if let Some(disk) = plan_state.disk_in_gb.trim().parse::<i64>().ok() {
                extras.insert("diskInGB".into(), Value::from(disk));
            }
            if let Some(bw) = plan_state.bandwidth_in_tb.trim().parse::<i64>().ok() {
                extras.insert("bandwidthInTB".into(), Value::from(bw));
            }
            if !extras.is_empty() {
                payload["extraResource"] = Value::Object(extras);
            }
        }
        let resp = api_call(&state, "POST", "/v1/instances", Some(payload), None).await;
        if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY")
            || resp.get("code").and_then(|c| c.as_str()) == Some("CREATED")
        {
            return Redirect::to("/instances").into_response();
        } else {
            // Build error / result page
            let mut errors: Vec<String> = Vec::new();
            if let Some(detail) = resp.get("detail").and_then(|d| d.as_str()) {
                if !detail.trim().is_empty() {
                    errors.push(detail.to_string());
                }
            }
            // Some APIs return 'errors' as array or map
            if let Some(arr) = resp.get("errors").and_then(|e| e.as_array()) {
                for entry in arr {
                    if let Some(s) = entry.as_str() {
                        errors.push(s.to_string());
                    } else if let Some(obj) = entry.as_object() {
                        for (k, v) in obj {
                            if let Some(s) = v.as_str() {
                                errors.push(format!("{}: {}", k, s));
                            } else {
                                errors.push(format!("{}: {}", k, value_to_short_string(v)));
                            }
                        }
                    } else {
                        errors.push(value_to_short_string(entry));
                    }
                }
            } else if let Some(obj) = resp.get("errors").and_then(|e| e.as_object()) {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        errors.push(format!("{}: {}", k, s));
                    } else {
                        errors.push(format!("{}: {}", k, value_to_short_string(v)));
                    }
                }
            }
            let code = resp.get("code").and_then(|c| c.as_str()).map(|s| s.to_string());
            let detail = resp.get("detail").and_then(|d| d.as_str()).map(|s| s.to_string());
            let raw_json = serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".into());
            let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
            return inject_context(
                &state,
                &jar,
                Step8Template {
                    current_user,
                    api_hostname,
                    base_url,
                    flash_messages,
                    has_flash_messages,
                    back_url: absolute_url(&state, "/create/step-6"),
                    status_label: "Failed".into(),
                    code,
                    detail,
                    errors,
                    raw_json,
                }
                .render()
                .unwrap(),
            );
        }
    }
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);
    let mut plan_summary = Vec::new();
    let mut price_entries = Vec::new();
    let mut footnote = None;
    
    // selected product details for review
    let mut selected_product_name: Option<String> = None;
    let mut selected_product_tags: Option<String> = None;
    let mut selected_product_description: Option<String> = None;
    
    if base.plan_type == "fixed" {
        let products = load_products(&state, &base.region).await;
        if let Some(prod) = products.into_iter().find(|p| p.id == plan_state.product_id) {
            plan_summary = prod.spec_entries.clone();
            price_entries = prod.price_entries.clone();
            let name = prod.name.clone();
            let tags = prod.tags.clone();
            let desc = prod.description.clone();
            if !desc.trim().is_empty() {
                footnote = Some(desc.clone());
            }
            // Create a friendly display name for the product: prefer plan name when useful,
            // otherwise fall back to tags, primary price, or first spec entry.
            let mut display_name: Option<String> = None;
            if !name.trim().is_empty() && name != prod.id {
                display_name = Some(name.clone());
            } else if !tags.trim().is_empty() {
                display_name = Some(tags.clone());
            } else if let Some(entry) = prod.price_entries.first() {
                display_name = Some(entry.value.clone());
            } else if let Some(entry) = prod.spec_entries.first() {
                display_name = Some(format!("{}: {}", entry.term, entry.value));
            }
            selected_product_name = display_name;
            selected_product_tags = if tags.trim().is_empty() { None } else { Some(tags.clone()) };
            selected_product_description = if desc.trim().is_empty() { None } else { Some(desc.clone()) };
        }
    } else {
        let mut summary = Vec::new();
        if !plan_state.cpu.trim().is_empty() {
            summary.push(ProductEntry {
                term: "vCPU".into(),
                value: plan_state.cpu.clone(),
            });
        }
        if !plan_state.ram_in_gb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "RAM (GB)".into(),
                value: plan_state.ram_in_gb.clone(),
            });
        }
        if !plan_state.disk_in_gb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "Disk (GB)".into(),
                value: plan_state.disk_in_gb.clone(),
            });
        }
        if !plan_state.bandwidth_in_tb.trim().is_empty() {
            summary.push(ProductEntry {
                term: "Bandwidth (TB)".into(),
                value: plan_state.bandwidth_in_tb.clone(),
            });
        }
        plan_summary = summary;
    }
    let os_list = load_os_list(&state).await;
    let selected_os_label = os_list
        .iter()
        .find(|os| os.id == base.os_id)
        .map(|os| {
            let mut label = os.name.clone();
            if let Some(version) = &os.version {
                if !version.is_empty() {
                    label.push(' ');
                    label.push_str(version);
                }
            }
            label
        })
        .unwrap_or_else(|| base.os_id.clone());
    let selected_key_ids: Vec<String> = base.ssh_key_ids.iter().map(|id| id.to_string()).collect();
    let ssh_keys_display = if selected_key_ids.is_empty() {
        "None".into()
    } else {
        let id_set: HashSet<_> = selected_key_ids.iter().cloned().collect();
        let customer_id = fetch_default_customer_id(&state).await;
        let ssh_keys = load_ssh_keys_api(&state, customer_id).await;
        let mut names = Vec::new();
        for key in ssh_keys {
            if id_set.contains(&key.id) {
                names.push(key.name);
            }
        }
        if names.is_empty() {
            format!("{} SSH key(s)", id_set.len())
        } else {
            names.join(", ")
        }
    };
    let hostnames_display = if base.hostnames.is_empty() {
        "(none)".into()
    } else {
        base.hostnames.join(", ")
    };
    let hostnames_csv = base.hostnames.join(",");
    let ssh_key_ids_csv = if selected_key_ids.is_empty() {
        "".into()
    } else {
        selected_key_ids.join(",")
    };
    let plan_type_label = if base.plan_type == "fixed" {
        "Fixed plan".into()
    } else {
        "Custom plan".into()
    };
    let mut back_pairs = build_base_query_pairs(&base);
    if base.plan_type == "fixed" {
        back_pairs.push(("product_id".into(), plan_state.product_id.clone()));
        back_pairs.push(("extra_disk".into(), plan_state.extra_disk.clone()));
        back_pairs.push(("extra_bandwidth".into(), plan_state.extra_bandwidth.clone()));
    } else {
        back_pairs.push(("cpu".into(), plan_state.cpu.clone()));
        back_pairs.push(("ramInGB".into(), plan_state.ram_in_gb.clone()));
        back_pairs.push(("diskInGB".into(), plan_state.disk_in_gb.clone()));
        back_pairs.push(("bandwidthInTB".into(), plan_state.bandwidth_in_tb.clone()));
    }
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        absolute_url(&state, "/create/step-6")
    } else {
        absolute_url(&state, &format!("/create/step-6?{}", back_q))
    };
    let has_plan_summary = !plan_summary.is_empty();
    let has_price_entries = !price_entries.is_empty();
    let footnote_text = footnote.unwrap_or_default();
    let has_footnote = !footnote_text.is_empty();
    inject_context(
        &state,
        &jar,
        Step7Template {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            base_state: &base,
            floating_ip_count: base.floating_ip_count.to_string(),
            plan_state,
            plan_type_label,
            region_name: base.region.clone(),
            hostnames_display,
            plan_summary,
            has_plan_summary,
            price_entries,
            has_price_entries,
            selected_os_label,
            ssh_keys_display,
            ssh_key_ids_csv,
            selected_product_name,
            selected_product_tags,
            selected_product_description,
            hostnames_csv,
            footnote_text,
            has_footnote,
            back_url,
            submit_url: absolute_url(&state, "/create/step-7"),
        }
        .render()
        .unwrap(),
    )
}

async fn create_step_7_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, OneOrMany>>,
) -> impl IntoResponse {
    // For GET requests, query params may have single or multiple values; flatten to CSV strings.
    let mut q_flat: HashMap<String, String> = HashMap::new();
    for (k, v) in q {
        q_flat.insert(k, v.to_csv());
    }
    create_step_7_core(state, jar, axum::http::Method::GET, q_flat, HashMap::new()).await
}

async fn create_step_8(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let code = q.get("code").cloned();
    let detail = q.get("detail").cloned();
    let raw_json = q.get("raw").cloned().unwrap_or_default();
    let errors = q.get("errors").map(|s| s.split('|').map(|s| s.to_string()).collect()).unwrap_or_else(Vec::new);
    inject_context(&state, &jar, Step8Template {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        back_url: q.get("back_url").cloned().unwrap_or_else(|| absolute_url(&state, "/create/step-1")),
        status_label: q.get("status_label").cloned().unwrap_or_else(|| "Result".into()),
        code,
        detail,
        errors,
        raw_json,
    }.render().unwrap())
}

async fn create_step_7_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, OneOrMany>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let mut q_flat: HashMap<String, String> = HashMap::new();
    for (k, v) in q {
        q_flat.insert(k, v.to_csv());
    }
    // Try to parse as HashMap<String, Vec<String>> first
    let mut f_flat: HashMap<String, String> = HashMap::new();
    let parsed_map = parse_urlencoded_body(&body);
    for (k, v) in parsed_map {
        f_flat.insert(k, v.join(","));
    }
    create_step_7_core(state, jar, axum::http::Method::POST, q_flat, f_flat).await
}

async fn api_call(
    state: &AppState,
    method: &str,
    endpoint: &str,
    data: Option<Value>,
    params: Option<Vec<(String, String)>>,
) -> Value {
    let url = format!("{}{}", state.api_base_url, endpoint);
    let should_log = !LOGGING_IGNORE_ENDPOINTS.contains(&endpoint);
    if should_log {
        tracing::info!(method, url, ?data, ?params, "API Request");
    }
    let mut req = match method {
        "GET" => state.client.get(&url),
        "POST" => state.client.post(&url),
        "DELETE" => state.client.delete(&url),
        _ => {
            return serde_json::json!({"code":"FAILED","detail":"Unsupported method","data":{}});
        }
    };
    req = req.header("API-Token", &state.api_token);
    if let Some(pairs) = &params {
        req = req.query(&pairs);
    }
    if let Some(body) = &data {
        req = req.json(body);
    }
    let resp = req.send().await;
    match resp {
        Ok(r) => {
            let status = r.status();
            let json_val = r
                .json::<Value>()
                .await
                .unwrap_or_else(|_| serde_json::json!({"raw":"non-json"}));
            if should_log {
                tracing::info!(%status, body=?json_val, "API Response");
            }
            json_val
        }
        Err(e) => {
            tracing::error!(error=%e, url, method, "API Error");
            serde_json::json!({"code":"FAILED","detail":format!("Network error: {}", e),"data":{}})
        }
    }
}
#[derive(Template)]
#[template(path = "users.html")]
struct UsersTemplate {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    rows: Vec<UserTableRow>,
}

struct UserTableRow {
    username: String,
    role: String,
    assigned: String,
}

#[derive(Deserialize)]
struct CreateUserForm {
    username: String,
    password: String,
    role: String,
}

fn ensure_owner(state: &AppState, jar: &CookieJar) -> Option<Redirect> {
    let uname = current_username_from_jar(state, jar)?;
    let users = state.users.lock().unwrap();
    let rec = users.get(&uname)?;
    if rec.role != "owner" {
        return Some(Redirect::to("/instances"));
    }
    None
}

async fn users_list(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let users = state.users.lock().unwrap();
    let mut rows: Vec<UserTableRow> = users
        .iter()
        .map(|(k, v)| {
            let assigned = if v.assigned_instances.is_empty() {
                String::new()
            } else {
                v.assigned_instances.join(", ")
            };
            UserTableRow {
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
    inject_context(
        &state,
        &jar,
        UsersTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            rows,
        }
        .render()
        .unwrap(),
    )
}

async fn users_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateUserForm>,
) -> impl IntoResponse {
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
        // persist
        let mut serialized: serde_json::Map<String, Value> = serde_json::Map::new();
        for (u, rec) in users.iter() {
            serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
        }
        let _ = std::fs::write(
            "users.json",
            serde_json::to_string_pretty(&Value::Object(serialized)).unwrap(),
        );
    }
    Redirect::to("/users").into_response()
}

#[derive(Deserialize)]
struct ResetPasswordForm {
    new_password: String,
}
async fn reset_password(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(username): axum::extract::Path<String>,
    Form(form): Form<ResetPasswordForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    if form.new_password.trim().is_empty() {
        return plain_html("Password cannot be empty");
    }
    let uname = username.to_lowercase();
    let mut users = state.users.lock().unwrap();
    if let Some(rec) = users.get_mut(&uname) {
        rec.password = generate_password_hash(&form.new_password);
    } else {
        return plain_html("User not found");
    }
    let mut serialized: serde_json::Map<String, Value> = serde_json::Map::new();
    for (u, rec) in users.iter() {
        serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
    }
    let _ = std::fs::write(
        "users.json",
        serde_json::to_string_pretty(&Value::Object(serialized)).unwrap(),
    );
    Redirect::to("/users").into_response()
}

#[derive(Deserialize)]
struct UpdateRoleForm {
    role: String,
}
async fn update_role(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(username): axum::extract::Path<String>,
    Form(form): Form<UpdateRoleForm>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let mut users = state.users.lock().unwrap();
    let uname = username.to_lowercase();
    if !["owner", "admin"].contains(&form.role.as_str()) {
        return plain_html("Invalid role");
    }
    if uname == current_username_from_jar(&state, &jar).unwrap_or_default() {
        return plain_html("Cannot change own role");
    }
    let Some(current_rec) = users.get(&uname) else {
        return plain_html("User not found");
    };
    if current_rec.role == "owner" && form.role != "owner" {
        let owners = users
            .iter()
            .filter(|(name, r)| r.role == "owner" && name.as_str() != uname)
            .count();
        if owners == 0 {
            return plain_html("At least one owner required");
        }
    }
    if let Some(rec) = users.get_mut(&uname) {
        rec.role = form.role.clone();
        if rec.role == "owner" {
            rec.assigned_instances.clear();
        }
    }
    let mut serialized: serde_json::Map<String, Value> = serde_json::Map::new();
    for (u, rec) in users.iter() {
        serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
    }
    let _ = std::fs::write(
        "users.json",
        serde_json::to_string_pretty(&Value::Object(serialized)).unwrap(),
    );
    Redirect::to("/users").into_response()
}

async fn delete_user(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let current = current_username_from_jar(&state, &jar).unwrap_or_default();
    let uname = username.to_lowercase();
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
    let mut serialized: serde_json::Map<String, Value> = serde_json::Map::new();
    for (u, rec) in users.iter() {
        serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
    }
    let _ = std::fs::write(
        "users.json",
        serde_json::to_string_pretty(&Value::Object(serialized)).unwrap(),
    );
    Redirect::to("/users").into_response()
}
#[derive(Template)]
#[template(path = "instances.html")]
struct InstancesTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    instances: &'a [InstanceView],
}

#[derive(Serialize, Deserialize, Clone)]
struct InstanceView {
    id: String,
    hostname: String,
}

async fn load_instances_for_user(state: &AppState, username: &str) -> Vec<InstanceView> {
    let payload = api_call(state, "GET", "/v1/instances", None, None).await;
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
                list.push(InstanceView { id, hostname });
            }
        }
    }
    let (role, allowed) = {
        let users = state.users.lock().unwrap();
        if let Some(user) = users.get(username) {
            if user.role == "admin" {
                (
                    Some(user.role.clone()),
                    Some(
                        user.assigned_instances
                            .iter()
                            .cloned()
                            .collect::<HashSet<String>>(),
                    ),
                )
            } else {
                (Some(user.role.clone()), None)
            }
        } else {
            (None, None)
        }
    };
    if role.as_deref() == Some("admin") {
        if let Some(allowed_set) = allowed {
            list.retain(|inst| allowed_set.contains(&inst.id));
        }
    }
    list
}

async fn instances_real(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let Some(username) = current_username_from_jar(&state, &jar) else {
        return Redirect::to("/login").into_response();
    };
    let list = load_instances_for_user(&state, &username).await;
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        InstancesTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instances: &list,
        }
        .render()
        .unwrap(),
    )
}

// Access management (owner only): list admins and assign instances
#[derive(Template)]
#[template(path = "access.html")]
struct AccessTemplate {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    admins: Vec<AdminView>,
}

struct AdminView {
    username: String,
    instances: Vec<AdminInstanceRow>,
}

struct AdminInstanceRow {
    id: String,
    hostname: String,
    checked: bool,
}

async fn access_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    // Load instances
    let payload = api_call(&state, "GET", "/v1/instances", None, None).await;
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
                list.push(InstanceView { id, hostname });
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
                    AdminInstanceRow {
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
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    inject_context(
        &state,
        &jar,
        AccessTemplate { current_user, api_hostname, base_url, flash_messages, has_flash_messages, admins }.render().unwrap(),
    )
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
        let mut serialized: serde_json::Map<String, Value> = serde_json::Map::new();
        for (u, r) in users.iter() {
            serialized.insert(u.clone(), serde_json::json!({"password": r.password, "role": r.role, "assigned_instances": r.assigned_instances }));
        }
        let _ = std::fs::write(
            "users.json",
            serde_json::to_string_pretty(&Value::Object(serialized)).unwrap(),
        );
    } else {
        return plain_html("Admin not found");
    }
    Redirect::to("/access").into_response()
}
// SSH Keys CRUD (owner only)
#[derive(Serialize, Deserialize, Clone)]
struct SshKeyView {
    id: String,
    name: String,
    fingerprint: String,
    public_key: String,
    customer_id: Option<String>,
}

#[derive(Template)]
#[template(path = "ssh_keys.html")]
struct SshKeysTemplate<'a> {
    current_user: Option<CurrentUser>,
    api_hostname: String,
    base_url: String,
    flash_messages: Vec<String>,
    has_flash_messages: bool,
    ssh_keys: &'a [SshKeyView],
    customer_id: Option<String>,
}

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

async fn fetch_default_customer_id(state: &AppState) -> Option<String> {
    if let Some(existing) = state.default_customer_cache.lock().unwrap().clone() {
        return Some(existing);
    }
    let endpoints = ["/v1/customers", "/v1/profile"];
    for endpoint in endpoints {
        let payload = api_call(state, "GET", endpoint, None, None).await;
        if let Some(id) = extract_customer_id_from_value(&payload) {
            let mut cache = state.default_customer_cache.lock().unwrap();
            *cache = Some(id.clone());
            return Some(id);
        }
    }
    None
}

async fn load_ssh_keys_api(state: &AppState, customer_id: Option<String>) -> Vec<SshKeyView> {
    let params = customer_id.map(|cid| vec![("customerId".to_string(), cid)]);
    let payload = api_call(state, "GET", "/v1/ssh-keys", None, params).await;
    if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
        return vec![];
    }
    let data = payload.get("data").cloned().unwrap_or(Value::Null);
    let candidates: Vec<Value> = if let Some(arr) = data.as_array() {
        arr.clone()
    } else if let Some(arr) = data.get("sshKeys").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        vec![]
    };
    let mut out = vec![];
    for item in candidates {
        if let Some(obj) = item.as_object() {
            let id = obj
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .or_else(|| {
                    obj.get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "0".into());
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("SSH Key {}", id));
            let fingerprint = obj
                .get("fingerprint")
                .or_else(|| obj.get("fingerPrint"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let public_key = obj
                .get("publicKey")
                .or_else(|| obj.get("public_key"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let customer_id = obj
                .get("customerId")
                .or_else(|| obj.get("userId"))
                .or_else(|| obj.get("customer_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            out.push(SshKeyView {
                id,
                name,
                fingerprint,
                public_key,
                customer_id,
            });
        }
    }
    out
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
    inject_context(
        &state,
        &jar,
        SshKeysTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            ssh_keys: &keys,
            customer_id,
        }
        .render()
        .unwrap(),
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
        let payload = api_call(&state, "DELETE", &endpoint, None, None).await;
        if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
            if let Some(detail) = payload.get("detail").and_then(|d| d.as_str()) {
                if detail_requires_customer(detail) {
                    if let Some(cid) = fetch_default_customer_id(&state).await {
                        let _ = api_call(
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
    let payload = api_call(&state, "POST", "/v1/ssh-keys", Some(body.clone()), None).await;
    if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
        if let Some(detail) = payload.get("detail").and_then(|d| d.as_str()) {
            if detail_requires_customer(detail) {
                if let Some(cid) = fetch_default_customer_id(&state).await {
                    body["customerId"] = Value::String(cid.clone());
                    let _ = api_call(&state, "POST", "/v1/ssh-keys", Some(body), None).await;
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

async fn root_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    // Redirect to /instances if authenticated, otherwise to /login
    if let Some(_uname) = current_username_from_jar(&state, &jar) {
        return Redirect::to("/instances").into_response();
    }
    Redirect::to("/login").into_response()
}

async fn regions_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let (regions, _) = load_regions(&state).await;
    // let total_regions = regions.len();
    // let premium_count = regions.iter().filter(|r| r.is_premium).count();
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let html = RegionsPageTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        regions: &regions,
    }
    .render()
    .unwrap();
    inject_context(&state, &jar, html)
}

async fn products_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let (regions, _) = load_regions(&state).await;
    let region_param = q
        .get("region_id")
        .cloned()
        .or_else(|| q.get("region").cloned())
        .or_else(|| q.get("regionId").cloned());
    let mut requested_region = None;
    let mut selected_region_idx: Option<usize> = None;
    if let Some(ref rid) = region_param {
        selected_region_idx = regions.iter().position(|r| r.id == rid.as_str());
        if selected_region_idx.is_none() {
            requested_region = Some(rid.clone());
        }
    } else if !regions.is_empty() {
        selected_region_idx = Some(0);
    }
    let selected_region_id = selected_region_idx.map(|idx| regions[idx].id.clone());
    let products = if let Some(ref region_id) = selected_region_id {
        load_products(&state, region_id).await
    } else {
        vec![]
    };
    let selected_region = selected_region_idx.map(|idx| &regions[idx]);
    let active_region_id = selected_region_id.clone().unwrap_or_default();
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let html = ProductsPageTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        regions: &regions,
        selected_region,
        active_region_id,
        requested_region,
        products: &products,
    }
    .render()
    .unwrap();
    inject_context(&state, &jar, html)
}

async fn os_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let os_list = load_os_list(&state).await;
    // let total_images = os_list.len();
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let html = OsCatalogTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        os_list: &os_list,
    }
    .render()
    .unwrap();
    inject_context(&state, &jar, html)
}

async fn applications_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let apps = load_applications(&state).await;
    // let total_apps = apps.len();
    let TemplateGlobals { current_user, api_hostname, base_url, flash_messages, has_flash_messages } = build_template_globals(&state, &jar);
    let html = ApplicationsTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        apps: &apps,
    }
    .render()
    .unwrap();
    inject_context(&state, &jar, html)
}

// ---------- Instance Detail & Actions ----------
// Instance details are rendered using `templates/instance_detail.html` (path-based Askama template)

async fn enforce_instance_access(state: &AppState, jar: &CookieJar, instance_id: &str) -> bool {
    if let Some(username) = current_username_from_jar(state, jar) {
        let users = state.users.lock().unwrap();
        if let Some(rec) = users.get(&username) {
            if rec.role == "owner" {
                return true;
            }
            return rec.assigned_instances.iter().any(|id| id == instance_id);
        }
    }
    false
}

async fn instance_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}", instance_id);
    let payload = api_call(&state, "GET", &endpoint, None, None).await;
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
                    let products = load_products(&state, &region).await;
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
    inject_context(
        &state,
        &jar,
        InstanceDetailTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            instance_id: instance_id.clone(),
            hostname,
            details,
        }
        .render()
        .unwrap(),
    )
}

async fn simple_instance_action(state: &AppState, action: &str, instance_id: &str) -> Value {
    let endpoint = format!("/v1/instances/{}/{}", instance_id, action);
    api_call(state, "POST", &endpoint, None, None).await
}

async fn instance_poweron(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let _ = simple_instance_action(&state, "poweron", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}
async fn instance_poweroff(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let _ = simple_instance_action(&state, "poweroff", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}
async fn instance_reset(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let _ = simple_instance_action(&state, "reset", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

async fn instance_change_pass(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let _ = simple_instance_action(&state, "change-pass", &instance_id).await;
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

#[derive(Deserialize)]
struct AddTrafficForm {
    traffic_amount: String,
}
async fn instance_add_traffic(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
    Form(form): Form<AddTrafficForm>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Ok(amount) = form.traffic_amount.parse::<f64>() {
        if amount > 0.0 {
            let endpoint = format!("/v1/instances/{}/add-traffic", instance_id);
            let payload = serde_json::json!({"amount": amount});
            let _ = api_call(&state, "POST", &endpoint, Some(payload), None).await;
        }
    }
    Redirect::to(&format!("/instance/{}", instance_id)).into_response()
}

// Simplified change-os (GET list, POST change)
async fn instance_change_os(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    if let Some(os_id) = q.get("os_id") {
        if !os_id.is_empty() {
            let endpoint = format!("/v1/instances/{}/change-os", instance_id);
            let payload = serde_json::json!({"osId": os_id});
            let _ = api_call(&state, "POST", &endpoint, Some(payload), None).await;
            return Redirect::to(&format!("/instance/{}", instance_id)).into_response();
        }
    }
    let os_list = load_os_list(&state).await;
    let mut html = String::from("<!DOCTYPE html><html><body><h1>Change OS</h1><ul>");
    for os in os_list {
        html.push_str(&format!(
            "<li><a href='/instance/{}/change-os?os_id={}'>{}</a></li>",
            instance_id, os.id, os.name
        ));
    }
    html.push_str(&format!(
        "</ul><p><a href='/instance/{}'>Back</a></p></body></html>",
        instance_id
    ));
    Html(html).into_response()
}

// Subscription refund
async fn instance_subscription_refund(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(instance_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !enforce_instance_access(&state, &jar, &instance_id).await {
        return Redirect::to("/instances").into_response();
    }
    let endpoint = format!("/v1/instances/{}/subscription-refund", instance_id);
    let payload = api_call(&state, "GET", &endpoint, None, None).await;
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
    let resp = api_call(
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
    Html(
        BulkRefundTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

#[derive(Parser)]
#[command(author, version, about = "Zyffiliate command-line tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web server
    Serve {
        /// Host to bind to
        #[arg(long, default_value_t = String::from("0.0.0.0"))]
        host: String,
        /// Port to bind to
        #[arg(long, default_value_t = 5000)]
        port: u16,
        /// Path to .env file
        #[arg(long)]
        env_file: Option<String>,
    },
    /// Validate configuration (env vars / API credentials)
    CheckConfig { env_file: Option<String> },
    /// Manage local users (users.json)
    Users {
        #[command(subcommand)]
        sub: UserCommands,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    List,
    Add {
        username: String,
        password: String,
        role: String,
    },
    /// Add a new owner user (use --force to overwrite existing owner user(s))
    AddOwner {
        username: String,
        password: String,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    ResetPassword {
        username: String,
        password: String,
    },
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
    // Build shared state (load users.json and env)
    let _user_store = {
        let path = std::path::Path::new("users.json");
        let mut map: HashMap<String, UserRecord> = HashMap::new();
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(obj) = json_val.as_object() {
                        for (k, v) in obj.iter() {
                            if let Some(pw) = v.get("password").and_then(|x| x.as_str()) {
                                let role = v
                                    .get("role")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("admin")
                                    .to_string();
                                let assigned_instances = v
                                    .get("assigned_instances")
                                    .and_then(|a| a.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_else(|| vec![]);
                                map.insert(
                                    k.to_lowercase(),
                                    UserRecord {
                                        password: pw.to_string(),
                                        role,
                                        assigned_instances,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        } else {
            // Create default owner user
            // Generate werkzeug compatible hash for 'owner123' using pbkdf2 parameters
            let salt = {
                let mut b = [0u8; 12];
                OsRng.fill_bytes(&mut b);
                hex_encode(b)
            };
            let mut dk = [0u8; 32];
            pbkdf2_hmac::<Sha256>(b"owner123", salt.as_bytes(), PBKDF2_ITERATIONS, &mut dk);
            let hash_hex = hex_encode(dk);
            let full = format!("pbkdf2:sha256:{}${}${}", PBKDF2_ITERATIONS, salt, hash_hex);
            map.insert(
                "owner".to_string(),
                UserRecord {
                    password: full,
                    role: "owner".to_string(),
                    assigned_instances: vec![],
                },
            );
            let mut serialized: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
            for (u, rec) in map.iter() {
                serialized.insert(u.clone(), serde_json::json!({"password": rec.password, "role": rec.role, "assigned_instances": rec.assigned_instances }));
            }
            let _ = std::fs::write(
                path,
                serde_json::to_string_pretty(&serde_json::Value::Object(serialized)).unwrap(),
            );
        }
        Arc::new(Mutex::new(map))
    };

    // Note: we avoid constructing a default `state` here; commands build the per-command state
    // using `build_state_from_env` so we can pass a custom `--env-file` when executing commands.

    // Dispatch CLI commands
    match cli.command.unwrap_or(Commands::Serve {
        host: String::from("0.0.0.0"),
        port: 5000,
        env_file: None,
    }) {
        Commands::Serve {
            host,
            port,
            env_file,
        } => {
            let state = build_state_from_env(env_file.as_deref());
            start_server(state, &host, port).await;
            return;
        }
        Commands::CheckConfig { env_file } => {
            let state = build_state_from_env(env_file.as_deref());
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
            let resp = api_call(&state, "GET", "/v1/regions", None, None).await;
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
            let state = build_state_from_env(None);
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
                    if let Err(e) = persist_users_file(&state.users) {
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
                    if let Err(e) = persist_users_file(&state.users) {
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
                    if let Err(e) = persist_users_file(&state.users) {
                        eprintln!("Failed to persist users.json: {}", e);
                        process::exit(1);
                    }
                    println!("Owner '{}' created", uname);
                    return;
                }
            }
        }
    }

    // All command arms either `return` or `process::exit`; nothing else to do here

    // (start_server handles starting the http listener)
}
