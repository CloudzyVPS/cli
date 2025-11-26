use askama::Template;
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use hex::encode as hex_encode;
use pbkdf2::pbkdf2_hmac;
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use urlencoding::encode;

const PBKDF2_ITERATIONS: u32 = 260_000;

const LOGGING_IGNORE_ENDPOINTS: &[&str] =
    &["/v1/regions", "/v1/products", "/v1/os", "/v1/ssh-keys"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserRecord {
    password: String, // werkzeug format: pbkdf2:sha256:ITERATIONS$salt$hash
    role: String,
    assigned_instances: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    users: Arc<Mutex<HashMap<String, UserRecord>>>,
    sessions: Arc<Mutex<HashMap<String, String>>>, // session_id -> username
    api_base_url: String,
    api_token: String,
    default_customer_cache: Arc<Mutex<Option<String>>>,
    client: Client,
}

fn parse_werkzeug_pbkdf2(hash: &str) -> Option<(u32, String, String)> {
    // Format: pbkdf2:sha256:iterations$salt$hash
    let parts: Vec<&str> = hash.split('$').collect();
    if parts.len() != 3 {
        return None;
    }
    let meta = parts[0];
    let salt = parts[1].to_string();
    let hash_hex = parts[2].to_string();
    let meta_parts: Vec<&str> = meta.split(':').collect();
    if meta_parts.len() != 3 {
        return None;
    }
    let iterations: u32 = meta_parts[2].parse().ok()?;
    Some((iterations, salt, hash_hex))
}

fn verify_password(stored: &str, candidate: &str) -> bool {
    if let Some((iterations, salt, expected_hex)) = parse_werkzeug_pbkdf2(stored) {
        let mut dk = [0u8; 32];
        pbkdf2_hmac::<Sha256>(candidate.as_bytes(), salt.as_bytes(), iterations, &mut dk);
        let computed = hex_encode(dk);
        computed == expected_hex
    } else {
        false
    }
}

fn random_session_id() -> String {
    let mut bytes = [0u8; 24];
    OsRng.fill_bytes(&mut bytes);
    hex_encode(bytes)
}

fn generate_password_hash(password: &str) -> String {
    let salt_bytes = {
        let mut b = [0u8; 12];
        OsRng.fill_bytes(&mut b);
        b
    };
    let salt_hex = hex_encode(salt_bytes);
    let mut dk = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        salt_hex.as_bytes(),
        PBKDF2_ITERATIONS,
        &mut dk,
    );
    let hash_hex = hex_encode(dk);
    format!(
        "pbkdf2:sha256:{}${}${}",
        PBKDF2_ITERATIONS, salt_hex, hash_hex
    )
}

fn render_stub(title: &str, path: &str) -> Html<String> {
    Html(format!("<!DOCTYPE html><html><head><title>Zyffiliate</title></head><body><h1>{}</h1><p>Stub page for {}</p></body></html>", title, path))
}

fn plain_html(msg: &str) -> Response {
    Html(msg.to_string()).into_response()
}

// Individual route handlers (stubs). Later these will load data & real templates.
#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Zyffiliate Login</title>
    <link rel="stylesheet" href="/static/styles.css" />
</head>
<body>
    <main class="auth">
        <section>
            <h1>Sign in</h1>
            {% if let Some(msg) = error %}
            <p class="error">{{ msg }}</p>
            {% endif %}
            <form method="post" action="/login">
                <label>Username
                    <input type="text" name="username" required />
                </label>
                <label>Password
                    <input type="password" name="password" required />
                </label>
                <button type="submit">Login</button>
            </form>
        </section>
    </main>
</body>
</html>"#,
    ext = "html"
)]
struct LoginTemplate<'a> {
    error: Option<&'a str>,
}

async fn login_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(username) = current_username_from_jar(&state, &jar) {
        let target = resolve_default_endpoint(&state, &username);
        return Redirect::to(&target).into_response();
    }
    inject_context(
        &state,
        &jar,
        LoginTemplate { error: None }.render().unwrap(),
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
    inject_context(
        &state,
        &jar,
        LoginTemplate {
            error: Some("Invalid credentials"),
        }
        .render()
        .unwrap(),
    )
}
async fn logout_post(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(sid) = jar.get("session_id").map(|c| c.value().to_string()) {
        state.sessions.lock().unwrap().remove(&sid);
    }
    let cleared = jar.remove(Cookie::from("session_id"));
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
    let sid = jar.get("session_id")?.value().to_string();
    state.sessions.lock().unwrap().get(&sid).cloned()
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

fn inject_context(state: &AppState, jar: &CookieJar, mut html: String) -> Response {
    let current = build_current_user(state, jar);
    let api_hostname = state.api_base_url.clone();
    // Insert a hidden context div right after opening <body>
    let ctx_div = format!("<div id='ctx' data-api-hostname='{}' data-current-username='{}' data-current-role='{}' style='display:none'></div>",
                          api_hostname,
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
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Create - Step 1</title>
    <link rel="stylesheet" href="/static/styles.css" />
</head>
<body>
    <header class="page-header">
        <p><a href="/instances">&larr; Back to dashboard</a></p>
        <h1>Select region and plan</h1>
        <p>Choose region, class, and billing model to kick off provisioning.</p>
    </header>
    <section>
        <form method="get" action="/create/step-2">
            <label>Region
                <select name="region">
                    {% for region in regions %}
                    <option value="{{ region.id }}" {% if region.id == form_region %}selected{% endif %}>{{ region.name }}</option>
                    {% endfor %}
                </select>
            </label>
            <label>Instance class
                <select name="instance_class">
                    <option value="default" {% if form_instance_class == "default" %}selected{% endif %}>default</option>
                    <option value="cpu-optimized" {% if form_instance_class == "cpu-optimized" %}selected{% endif %}>cpu-optimized</option>
                </select>
            </label>
            <label>Plan type
                <select name="plan_type">
                    <option value="fixed" {% if form_plan_type == "fixed" %}selected{% endif %}>fixed</option>
                    <option value="custom" {% if form_plan_type == "custom" %}selected{% endif %}>custom</option>
                </select>
            </label>
            <button type="submit">Next</button>
        </form>
    </section>
</body>
</html>"#,
    ext = "html"
)]
struct Step1Template<'a> {
    regions: &'a [Region],
    form_region: &'a str,
    form_instance_class: &'a str,
    form_plan_type: &'a str,
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
    inject_context(
        &state,
        &jar,
        Step1Template {
            regions: &regions,
            form_region: &region_sel,
            form_instance_class: &base.instance_class,
            form_plan_type: &base.plan_type,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 2 (Hostnames & IP Assignment) ----------
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Step 2</title></head>
<body>
    <h1>Hostnames & Networking</h1>
    <form method="get" action="/create/step-3">
        <textarea name="hostnames" rows="4" cols="40">{{ hostnames_text }}</textarea>
        <div>
            <label><input type="checkbox" name="assign_ipv4" value="1" {% if base.assign_ipv4 %}checked{% endif %}/> Assign IPv4</label>
            <label><input type="checkbox" name="assign_ipv6" value="1" {% if base.assign_ipv6 %}checked{% endif %}/> Assign IPv6</label>
        </div>
        <label>Floating IP count <input type="number" name="floating_ip_count" value="{{ base.floating_ip_count }}" /></label>
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <button type="submit">Next</button>
    </form>
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
struct Step2Template<'a> {
    base: &'a BaseState,
    hostnames_text: String,
    back_url: String,
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
        "/create/step-1".to_string()
    } else {
        format!("/create/step-1?{}", back_q)
    };
    let hostnames_text = base.hostnames.join(", ");
    inject_context(
        &state,
        &jar,
        Step2Template {
            base: &base,
            hostnames_text,
            back_url,
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
    description: String,
    tags: String,
    spec_entries: Vec<ProductEntry>,
    price_entries: Vec<ProductEntry>,
}

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Step 3</title></head>
<body>
    <h1>Plan details</h1>
    <form method="get" action="/create/step-4">
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <input type="hidden" name="hostnames" value="{{ hostnames_join }}" />
        {% if base.plan_type == "fixed" %}
            {% for product in products %}
            <label><input type="radio" name="product_id" value="{{ product.id }}" {% if product.id == selected_product_id %}checked{% endif %}/> {{ product.name }} - {{ product.tags }}</label><br />
            {% endfor %}
        {% else %}
            <label>CPU <input type="number" name="cpu" value="{{ cpu }}" /></label>
            <label>RAM (GB) <input type="number" name="ramInGB" value="{{ ramInGB }}" /></label>
            <label>Disk (GB) <input type="number" name="diskInGB" value="{{ diskInGB }}" /></label>
            <label>Bandwidth (TB) <input type="number" name="bandwidthInTB" value="{{ bandwidthInTB }}" step="0.1" /></label>
        {% endif %}
        <button type="submit">Next</button>
    </form>
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
#[allow(non_snake_case)]
struct Step3Template<'a> {
    base: &'a BaseState,
    products: &'a [ProductView],
    selected_product_id: &'a str,
    hostnames_join: String,
    cpu: String,
    ramInGB: String,
    diskInGB: String,
    bandwidthInTB: String,
    back_url: String,
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
                    out.push(ProductView {
                        id,
                        name,
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
        "/create/step-2".to_string()
    } else {
        format!("/create/step-2?{}", back_q)
    };
    if base.plan_type == "fixed" {
        let products = load_products(&state, &base.region).await;
        let selected_product_id = q.get("product_id").map(|s| s.as_str()).unwrap_or("");
        let hostnames_join = base.hostnames.join(", ");
        return inject_context(
            &state,
            &jar,
            Step3Template {
                base: &base,
                products: &products,
                selected_product_id,
                hostnames_join,
                cpu: "".into(),
                ramInGB: "".into(),
                diskInGB: "".into(),
                bandwidthInTB: "".into(),
                back_url,
            }
            .render()
            .unwrap(),
        );
    }
    let hostnames_join = base.hostnames.join(", ");
    let cpu = q.get("cpu").cloned().unwrap_or_default();
    let ram = q.get("ramInGB").cloned().unwrap_or_default();
    let disk = q.get("diskInGB").cloned().unwrap_or_default();
    let bw = q.get("bandwidthInTB").cloned().unwrap_or_default();
    inject_context(
        &state,
        &jar,
        Step3Template {
            base: &base,
            products: &[],
            selected_product_id: "",
            hostnames_join,
            cpu,
            ramInGB: ram,
            diskInGB: disk,
            bandwidthInTB: bw,
            back_url,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 4 (Extras for fixed plans) ----------
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Step 4</title></head>
<body>
    <h1>Extras</h1>
    {% if base.plan_type == "fixed" %}
    <form method="get" action="/create/step-5">
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <input type="hidden" name="hostnames" value="{{ hostnames_join }}" />
        <input type="hidden" name="product_id" value="{{ product_id }}" />
        <label>Extra disk (GB) <input type="number" name="extra_disk" value="{{ extra_disk }}" /></label>
        <label>Extra bandwidth (TB) <input type="number" name="extra_bandwidth" value="{{ extra_bandwidth }}" step="0.1" /></label>
        <button type="submit">Next</button>
    </form>
    {% else %}
    <p>No extras for custom plans.</p>
    <p><a href="{{ next_url }}">Continue</a></p>
    {% endif %}
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
#[allow(non_snake_case)]
struct Step4Template<'a> {
    base: &'a BaseState,
    hostnames_join: String,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    back_url: String,
    next_url: String,
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
    let hostnames_join = base.hostnames.join(", ");
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if back_q.is_empty() {
        "/create/step-3".to_string()
    } else {
        format!("/create/step-3?{}", back_q)
    };
    if base.plan_type != "fixed" {
        // Skip extras
        let next_pairs = build_base_query_pairs(&base);
        let next_q = build_query_string(&next_pairs);
        let next_url = format!("/create/step-5?{}", next_q);
        return inject_context(
            &state,
            &jar,
            Step4Template {
                base: &base,
                hostnames_join,
                product_id: String::new(),
                extra_disk: String::new(),
                extra_bandwidth: String::new(),
                back_url,
                next_url,
            }
            .render()
            .unwrap(),
        );
    }
    let product_id = q.get("product_id").cloned().unwrap_or_default();
    if product_id.is_empty() {
        return Redirect::to("/create/step-3").into_response();
    }
    let extra_disk = q.get("extra_disk").cloned().unwrap_or("0".into());
    let extra_bandwidth = q.get("extra_bandwidth").cloned().unwrap_or("0".into());
    inject_context(
        &state,
        &jar,
        Step4Template {
            base: &base,
            hostnames_join,
            product_id,
            extra_disk,
            extra_bandwidth,
            back_url,
            next_url: String::new(),
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

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Step 5</title></head>
<body>
    <h1>Select operating system</h1>
    <form method="get" action="/create/step-6">
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <input type="hidden" name="hostnames" value="{{ hostnames_join }}" />
        {% if base.plan_type == "fixed" %}
            <input type="hidden" name="product_id" value="{{ product_id }}" />
            <input type="hidden" name="extra_disk" value="{{ extra_disk }}" />
            <input type="hidden" name="extra_bandwidth" value="{{ extra_bandwidth }}" />
        {% else %}
            <input type="hidden" name="cpu" value="{{ cpu }}" />
            <input type="hidden" name="ramInGB" value="{{ ramInGB }}" />
            <input type="hidden" name="diskInGB" value="{{ diskInGB }}" />
            <input type="hidden" name="bandwidthInTB" value="{{ bandwidthInTB }}" />
        {% endif %}
        {% for os in os_list %}
            <label><input type="radio" name="os_id" value="{{ os.id }}" {% if os.id == selected_os_id %}checked{% endif %}/> {{ os.name }} ({{ os.family }})</label><br />
        {% endfor %}
        <button type="submit">Next</button>
    </form>
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
#[allow(non_snake_case)]
struct Step5Template<'a> {
    base: &'a BaseState,
    hostnames_join: String,
    os_list: &'a [OsItem],
    selected_os_id: &'a str,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    cpu: String,
    ramInGB: String,
    diskInGB: String,
    bandwidthInTB: String,
    back_url: String,
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
    let hostnames_join = base.hostnames.join(", ");
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = if base.plan_type == "fixed" {
        format!("/create/step-4?{}", back_q)
    } else {
        format!("/create/step-3?{}", back_q)
    };
    let os_list = load_os_list(&state).await;
    let selected_os_id = q.get("os_id").cloned().unwrap_or_else(|| {
        os_list
            .iter()
            .find(|o| o.is_default)
            .map(|o| o.id.clone())
            .unwrap_or_default()
    });
    inject_context(
        &state,
        &jar,
        Step5Template {
            base: &base,
            hostnames_join,
            os_list: &os_list,
            selected_os_id: &selected_os_id,
            product_id: q.get("product_id").cloned().unwrap_or_default(),
            extra_disk: q.get("extra_disk").cloned().unwrap_or("0".into()),
            extra_bandwidth: q.get("extra_bandwidth").cloned().unwrap_or("0".into()),
            cpu: q.get("cpu").cloned().unwrap_or_default(),
            ramInGB: q.get("ramInGB").cloned().unwrap_or_default(),
            diskInGB: q.get("diskInGB").cloned().unwrap_or_default(),
            bandwidthInTB: q.get("bandwidthInTB").cloned().unwrap_or_default(),
            back_url,
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
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Step 6</title></head>
<body>
    <h1>Choose SSH keys</h1>
    <form method="get" action="/create/step-7">
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <input type="hidden" name="hostnames" value="{{ hostnames_join }}" />
        <input type="hidden" name="os_id" value="{{ base.os_id }}" />
        {% if base.plan_type == "fixed" %}
            <input type="hidden" name="product_id" value="{{ product_id }}" />
            <input type="hidden" name="extra_disk" value="{{ extra_disk }}" />
            <input type="hidden" name="extra_bandwidth" value="{{ extra_bandwidth }}" />
        {% else %}
            <input type="hidden" name="cpu" value="{{ cpu }}" />
            <input type="hidden" name="ramInGB" value="{{ ramInGB }}" />
            <input type="hidden" name="diskInGB" value="{{ diskInGB }}" />
            <input type="hidden" name="bandwidthInTB" value="{{ bandwidthInTB }}" />
        {% endif %}
        {% for key in ssh_keys %}
            <label><input type="checkbox" name="ssh_key_ids" value="{{ key.id }}" {% if key.selected %}checked{% endif %}/> {{ key.name }}</label><br />
        {% endfor %}
        <button type="submit">Next</button>
    </form>
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
#[allow(non_snake_case)]
struct Step6Template<'a> {
    base: &'a BaseState,
    hostnames_join: String,
    ssh_keys: Vec<SelectableSshKey>,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    cpu: String,
    ramInGB: String,
    diskInGB: String,
    bandwidthInTB: String,
    back_url: String,
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
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
    }
    let hostnames_join = base.hostnames.join(", ");
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = format!("/create/step-5?{}", back_q);
    let customer_id = fetch_default_customer_id(&state).await;
    let ssh_keys = load_ssh_keys_api(&state, customer_id).await;
    let selected_ids: std::collections::HashSet<String> =
        base.ssh_key_ids.iter().map(|id| id.to_string()).collect();
    let selectable: Vec<SelectableSshKey> = ssh_keys
        .into_iter()
        .map(|key| {
            let selected = selected_ids.contains(&key.id);
            SelectableSshKey {
                id: key.id,
                name: key.name,
                selected,
            }
        })
        .collect();
    inject_context(
        &state,
        &jar,
        Step6Template {
            base: &base,
            hostnames_join,
            ssh_keys: selectable,
            product_id: q.get("product_id").cloned().unwrap_or_default(),
            extra_disk: q.get("extra_disk").cloned().unwrap_or("0".into()),
            extra_bandwidth: q.get("extra_bandwidth").cloned().unwrap_or("0".into()),
            cpu: q.get("cpu").cloned().unwrap_or_default(),
            ramInGB: q.get("ramInGB").cloned().unwrap_or_default(),
            diskInGB: q.get("diskInGB").cloned().unwrap_or_default(),
            bandwidthInTB: q.get("bandwidthInTB").cloned().unwrap_or_default(),
            back_url,
        }
        .render()
        .unwrap(),
    )
}

// ---------- Wizard Step 7 (Review & Create) ----------
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Create - Review</title></head>
<body>
    <h1>Review configuration</h1>
    <p>Region: {{ base.region }}</p>
    <p>Hostnames: {{ hostnames_join }}</p>
    <p>Plan type: {{ base.plan_type }}</p>
    <p>Instance class: {{ base.instance_class }}</p>
    <p>OS: {{ base.os_id }}</p>
    <form method="post">
        <input type="hidden" name="region" value="{{ base.region }}" />
        <input type="hidden" name="instance_class" value="{{ base.instance_class }}" />
        <input type="hidden" name="plan_type" value="{{ base.plan_type }}" />
        <input type="hidden" name="hostnames" value="{{ hostnames_join }}" />
        <input type="hidden" name="os_id" value="{{ base.os_id }}" />
        {% for id in base.ssh_key_ids %}
            <input type="hidden" name="ssh_key_ids" value="{{ id }}" />
        {% endfor %}
        <input type="hidden" name="product_id" value="{{ product_id }}" />
        <input type="hidden" name="extra_disk" value="{{ extra_disk }}" />
        <input type="hidden" name="extra_bandwidth" value="{{ extra_bandwidth }}" />
        <input type="hidden" name="cpu" value="{{ cpu }}" />
        <input type="hidden" name="ramInGB" value="{{ ramInGB }}" />
        <input type="hidden" name="diskInGB" value="{{ diskInGB }}" />
        <input type="hidden" name="bandwidthInTB" value="{{ bandwidthInTB }}" />
        <button type="submit">Create instance</button>
    </form>
    <p><a href="{{ back_url }}">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
#[allow(non_snake_case)]
struct Step7Template<'a> {
    base: &'a BaseState,
    hostnames_join: String,
    product_id: String,
    extra_disk: String,
    extra_bandwidth: String,
    cpu: String,
    ramInGB: String,
    diskInGB: String,
    bandwidthInTB: String,
    back_url: String,
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
    if base.os_id.is_empty() {
        return Redirect::to("/create/step-5").into_response();
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
            if let Some(prod) = source.get("product_id") {
                payload["productId"] = Value::from(prod.clone());
            }
            let mut extras = serde_json::Map::new();
            if let Some(d) = source.get("extra_disk").and_then(|v| v.parse::<i64>().ok()) {
                if d > 0 {
                    extras.insert("diskInGB".into(), Value::from(d));
                }
            }
            if let Some(b) = source
                .get("extra_bandwidth")
                .and_then(|v| v.parse::<i64>().ok())
            {
                if b > 0 {
                    extras.insert("bandwidthInTB".into(), Value::from(b));
                }
            }
            if !extras.is_empty() {
                payload["extraResource"] = Value::Object(extras);
            }
        } else {
            let mut extras = serde_json::Map::new();
            if let Some(cpu) = source.get("cpu").and_then(|v| v.parse::<i64>().ok()) {
                extras.insert("cpu".into(), Value::from(cpu));
            }
            if let Some(ram) = source.get("ramInGB").and_then(|v| v.parse::<i64>().ok()) {
                extras.insert("ramInGB".into(), Value::from(ram));
            }
            if let Some(disk) = source.get("diskInGB").and_then(|v| v.parse::<i64>().ok()) {
                extras.insert("diskInGB".into(), Value::from(disk));
            }
            if let Some(bw) = source
                .get("bandwidthInTB")
                .and_then(|v| v.parse::<i64>().ok())
            {
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
        }
    }
    let back_pairs = build_base_query_pairs(&base);
    let back_q = build_query_string(&back_pairs);
    let back_url = format!("/create/step-6?{}", back_q);
    let hostnames_join = base.hostnames.join(", ");
    let product_id = query
        .get("product_id")
        .cloned()
        .or_else(|| form.get("product_id").cloned())
        .unwrap_or_default();
    let extra_disk = query
        .get("extra_disk")
        .cloned()
        .or_else(|| form.get("extra_disk").cloned())
        .unwrap_or_else(|| "0".into());
    let extra_bandwidth = query
        .get("extra_bandwidth")
        .cloned()
        .or_else(|| form.get("extra_bandwidth").cloned())
        .unwrap_or_else(|| "0".into());
    let cpu = query
        .get("cpu")
        .cloned()
        .or_else(|| form.get("cpu").cloned())
        .unwrap_or_default();
    let ram = query
        .get("ramInGB")
        .cloned()
        .or_else(|| form.get("ramInGB").cloned())
        .unwrap_or_default();
    let disk = query
        .get("diskInGB")
        .cloned()
        .or_else(|| form.get("diskInGB").cloned())
        .unwrap_or_default();
    let bandwidth = query
        .get("bandwidthInTB")
        .cloned()
        .or_else(|| form.get("bandwidthInTB").cloned())
        .unwrap_or_default();
    inject_context(
        &state,
        &jar,
        Step7Template {
            base: &base,
            hostnames_join,
            product_id,
            extra_disk,
            extra_bandwidth,
            cpu,
            ramInGB: ram,
            diskInGB: disk,
            bandwidthInTB: bandwidth,
            back_url,
        }
        .render()
        .unwrap(),
    )
}

async fn create_step_7_get(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    create_step_7_core(state, jar, axum::http::Method::GET, q, HashMap::new()).await
}

async fn create_step_7_post(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    create_step_7_core(state, jar, axum::http::Method::POST, q, form).await
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
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Users</title></head>
<body>
    <h1>User accounts</h1>
    {% if let Some(msg) = message %}
    <p>{{ msg }}</p>
    {% endif %}
    <table border="1">
        <tr><th>Username</th><th>Role</th><th>Assigned</th></tr>
        {% for row in rows %}
        <tr><td>{{ row.username }}</td><td>{{ row.role }}</td><td>{{ row.assigned }}</td></tr>
        {% endfor %}
    </table>
</body>
</html>"#,
    ext = "html"
)]
struct UsersTemplate<'a> {
    rows: Vec<UserTableRow>,
    message: Option<&'a str>,
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
    inject_context(
        &state,
        &jar,
        UsersTemplate {
            rows,
            message: None,
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
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Instances</title></head>
<body>
    <h1>Instances</h1>
    <ul>
        {% for inst in instances %}
        <li>#{{ inst.id }} {{ inst.hostname }}</li>
        {% endfor %}
    </ul>
</body>
</html>"#,
    ext = "html"
)]
struct InstancesTemplate<'a> {
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
                            .collect::<std::collections::HashSet<String>>(),
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
    inject_context(
        &state,
        &jar,
        InstancesTemplate { instances: &list }.render().unwrap(),
    )
}

// Access management (owner only): list admins and assign instances
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Access</title></head>
<body>
    <h1>Admin assignments</h1>
    {% for admin in admins %}
    <section>
        <h2>{{ admin.username }}</h2>
        <form method="post" action="/access/{{ admin.username }}">
            {% for inst in admin.instances %}
            <label><input type="checkbox" name="instances" value="{{ inst.id }}" {% if inst.checked %}checked{% endif %}/> #{{ inst.id }} {{ inst.hostname }}</label><br />
            {% endfor %}
            <button type="submit">Save</button>
        </form>
    </section>
    {% endfor %}
</body>
</html>"#,
    ext = "html"
)]
struct AccessTemplate {
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
            let assigned: std::collections::HashSet<&str> =
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
    inject_context(&state, &jar, AccessTemplate { admins }.render().unwrap())
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
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>SSH Keys</title></head>
<body>
    <h1>SSH Keys</h1>
    {% if let Some(msg) = error %}
    <p style="color:red">{{ msg }}</p>
    {% endif %}
    <form method="post">
        <input type="hidden" name="action" value="create" />
        <label>Name <input type="text" name="name" /></label>
        <label>Public key <textarea name="public_key"></textarea></label>
        <button type="submit">Add</button>
    </form>
    <ul>
        {% for key in ssh_keys %}
        <li>#{{ key.id }} {{ key.name }}
            <form method="post" style="display:inline">
                <input type="hidden" name="action" value="delete" />
                <input type="hidden" name="ssh_key_id" value="{{ key.id }}" />
                <button type="submit">Delete</button>
            </form>
        </li>
        {% endfor %}
    </ul>
</body>
</html>"#,
    ext = "html"
)]
struct SshKeysTemplate<'a> {
    ssh_keys: &'a [SshKeyView],
    error: Option<&'a str>,
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
    let keys = load_ssh_keys_api(&state, customer_id).await;
    inject_context(
        &state,
        &jar,
        SshKeysTemplate {
            ssh_keys: &keys,
            error: None,
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

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>Regions</title>
    <link rel="stylesheet" href="/static/styles.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
    <main>
        <header class="page-header">
            <p><a href="/instances">&larr; Back to dashboard</a></p>
            <h1>Regions</h1>
            <p>Active infrastructure regions provided by the Cloudzy API.</p>
            <p class="page-status">{{ total_regions }} active region{% if total_regions != 1 %}s{% endif %} • {{ premium_count }} premium</p>
        </header>
        {% if regions.is_empty() %}
        <section>
            <p>No regions were returned.</p>
        </section>
        {% else %}
        <section aria-labelledby="regions-heading">
            <h2 id="regions-heading" class="sr-only">Region list</h2>
            <div class="region-grid" role="list">
                {% for region in regions %}
                <article class="region-card" role="listitem">
                    <div class="region-heading">
                        <h3>{{ region.name }}</h3>
                        {% if let Some(code) = region.abbr.as_ref() %}
                        <span class="region-chip">{{ code }}</span>
                        {% endif %}
                    </div>
                    <p class="region-subtitle">Region ID {{ region.id }}</p>
                    {% if let Some(desc) = region.description.as_ref() %}
                    <p class="region-description">{{ desc }}</p>
                    {% endif %}
                    <dl class="region-metrics">
                        <div>
                            <dt>Status</dt>
                            <dd>{% if region.is_active %}Active{% else %}Disabled{% endif %}</dd>
                        </div>
                        <div>
                            <dt>Premium tier</dt>
                            <dd>{% if region.is_premium %}Premium{% else %}Standard{% endif %}</dd>
                        </div>
                        {% if let Some(ram) = region.ram_threshold_gb %}
                        <div>
                            <dt>RAM threshold</dt>
                            <dd>{{ ram }} GB</dd>
                        </div>
                        {% endif %}
                        {% if let Some(disk) = region.disk_threshold_gb %}
                        <div>
                            <dt>Disk threshold</dt>
                            <dd>{{ disk }} GB</dd>
                        </div>
                        {% endif %}
                    </dl>
                    {% if let Some(tags) = region.tags.as_ref() %}
                    <footer class="region-footer">
                        <p>{{ tags }}</p>
                    </footer>
                    {% endif %}
                    <p class="region-cta"><a href="/products?region_id={{ region.id }}">Browse products &rarr;</a></p>
                </article>
                {% endfor %}
            </div>
        </section>
        {% endif %}
    </main>
</body>
</html>"#,
    ext = "html"
)]
struct RegionsPageTemplate<'a> {
    regions: &'a [Region],
    total_regions: usize,
    premium_count: usize,
}

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>Products</title>
    <link rel="stylesheet" href="/static/styles.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
    <main>
        <header class="page-header">
            <p><a href="/regions">&larr; Back to regions</a></p>
            <h1>Products</h1>
            {% if let Some(region) = selected_region %}
            <p>Plans available in <strong>{{ region.name }}</strong> ({{ region.id }}).</p>
            {% else %}
            <p>Select a region to load its catalog.</p>
            {% endif %}
            {% if let Some(rid) = requested_region.as_ref() %}
            <p class="page-status warning">No region with id {{ rid }} was found.</p>
            {% endif %}
        </header>
        <section>
            <h2>Select a region</h2>
            {% if regions.is_empty() %}
            <p>No regions are available yet.</p>
            {% else %}
            <div class="region-grid" role="list">
                {% for region in regions %}
                <a class="region-card" role="listitem" href="/products?region_id={{ region.id }}" {% if !active_region_id.is_empty() && active_region_id == region.id %}aria-current="true"{% endif %}>
                    <div class="region-heading">
                        <h3>{{ region.name }}</h3>
                        {% if let Some(code) = region.abbr.as_ref() %}
                        <span class="region-chip">{{ code }}</span>
                        {% endif %}
                    </div>
                    <p class="region-subtitle">Region ID {{ region.id }}</p>
                    {% if let Some(desc) = region.description.as_ref() %}
                    <p class="region-description">{{ desc }}</p>
                    {% endif %}
                    <dl class="region-metrics">
                        <div>
                            <dt>Premium</dt>
                            <dd>{% if region.is_premium %}Yes{% else %}No{% endif %}</dd>
                        </div>
                        {% if let Some(ram) = region.ram_threshold_gb %}
                        <div>
                            <dt>RAM threshold</dt>
                            <dd>{{ ram }} GB</dd>
                        </div>
                        {% endif %}
                        {% if let Some(disk) = region.disk_threshold_gb %}
                        <div>
                            <dt>Disk threshold</dt>
                            <dd>{{ disk }} GB</dd>
                        </div>
                        {% endif %}
                    </dl>
                    <span class="region-cta" aria-hidden="true">{% if !active_region_id.is_empty() && active_region_id == region.id %}Viewing{% else %}View{% endif %} products</span>
                </a>
                {% endfor %}
            </div>
            {% endif %}
        </section>
        <section>
            <h2>Product catalog</h2>
            {% if let Some(region) = selected_region %}
                {% if products.is_empty() %}
                <p>📦 No products were returned for {{ region.name }}.</p>
                {% else %}
                <div class="product-grid" role="list">
                    {% for product in products %}
                    <article class="product-card" role="listitem" data-view-only="true">
                        <div class="product-card-body">
                            <header>
                                <p class="product-plan-name">Product #{{ product.id }}</p>
                                <h2>{{ product.name }}</h2>
                            </header>
                            {% if !product.description.is_empty() %}
                            <p>{{ product.description }}</p>
                            {% endif %}
                            {% if !product.spec_entries.is_empty() %}
                            <dl>
                                {% for entry in product.spec_entries %}
                                <div>
                                    <dt>{{ entry.term }}</dt>
                                    <dd>{{ entry.value }}</dd>
                                </div>
                                {% endfor %}
                            </dl>
                            {% endif %}
                            {% if !product.price_entries.is_empty() %}
                            <dl class="pricing">
                                {% for entry in product.price_entries %}
                                <div>
                                    <dt>{{ entry.term }}</dt>
                                    <dd>{{ entry.value }}</dd>
                                </div>
                                {% endfor %}
                            </dl>
                            {% endif %}
                            {% if !product.tags.is_empty() %}
                            <footer>
                                <p>{{ product.tags }}</p>
                            </footer>
                            {% endif %}
                        </div>
                    </article>
                    {% endfor %}
                </div>
                {% endif %}
            {% else %}
            <p>Select a region above to preview product plans.</p>
            {% endif %}
        </section>
    </main>
</body>
</html>"#,
    ext = "html"
)]
struct ProductsPageTemplate<'a> {
    regions: &'a [Region],
    selected_region: Option<&'a Region>,
    active_region_id: String,
    requested_region: Option<String>,
    products: &'a [ProductView],
}

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>Operating systems</title>
    <link rel="stylesheet" href="/static/styles.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
    <main>
        <header class="page-header">
            <p><a href="/instances">&larr; Back to dashboard</a></p>
            <h1>Operating systems</h1>
            <p>{{ total_images }} images available for provisioning.</p>
        </header>
        <section>
            <h2>Image catalog</h2>
            {% if os_list.is_empty() %}
            <p>💿 No operating systems were returned.</p>
            {% else %}
            <div class="data-grid">
                {% for os in os_list %}
                <article class="data-card">
                    <header>
                        <h3>{{ os.name }}</h3>
                        <p class="os-family">{{ os.family }}</p>
                    </header>
                    <dl class="os-attributes">
                        {% if let Some(arch) = os.arch.as_ref() %}
                        <div>
                            <dt>Architecture</dt>
                            <dd>{{ arch }}</dd>
                        </div>
                        {% endif %}
                        {% if let Some(version) = os.version.as_ref() %}
                        <div>
                            <dt>Version</dt>
                            <dd>{{ version }}</dd>
                        </div>
                        {% endif %}
                        {% if let Some(ram) = os.min_ram.as_ref() %}
                        <div>
                            <dt>Min RAM</dt>
                            <dd>{{ ram }}</dd>
                        </div>
                        {% endif %}
                        {% if let Some(disk) = os.disk.as_ref() %}
                        <div>
                            <dt>Disk</dt>
                            <dd>{{ disk }}</dd>
                        </div>
                        {% endif %}
                        <div>
                            <dt>Default</dt>
                            <dd>{% if os.is_default %}Yes{% else %}No{% endif %}</dd>
                        </div>
                    </dl>
                    {% if let Some(desc) = os.description.as_ref() %}
                    <p class="os-description">{{ desc }}</p>
                    {% endif %}
                </article>
                {% endfor %}
            </div>
            {% endif %}
        </section>
    </main>
</body>
</html>"#,
    ext = "html"
)]
struct OsCatalogTemplate<'a> {
    os_list: &'a [OsItem],
    total_images: usize,
}

#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>Applications</title>
    <link rel="stylesheet" href="/static/styles.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
    <main>
        <header class="page-header">
            <p><a href="/instances">&larr; Back to dashboard</a></p>
            <h1>Applications</h1>
            <p>{{ total_apps }} catalog item{% if total_apps != 1 %}s{% endif %} retrieved from the API.</p>
        </header>
        <section>
            <h2>Marketplace</h2>
            {% if apps.is_empty() %}
            <p>No applications were returned.</p>
            {% else %}
            <div class="data-grid">
                {% for app in apps %}
                <article class="data-card">
                    <header>
                        <h3>{{ app.name }}</h3>
                        {% if let Some(category) = app.category.as_ref() %}
                        <p class="os-family">{{ category }}</p>
                        {% endif %}
                    </header>
                    <p>{{ app.description }}</p>
                    <dl>
                        <div>
                            <dt>Application ID</dt>
                            <dd>{{ app.id }}</dd>
                        </div>
                        {% if let Some(price) = app.price.as_ref() %}
                        <div>
                            <dt>Price</dt>
                            <dd>{{ price }}</dd>
                        </div>
                        {% endif %}
                        <div>
                            <dt>Featured</dt>
                            <dd>{% if app.is_featured %}Yes{% else %}No{% endif %}</dd>
                        </div>
                    </dl>
                    {% if let Some(tags) = app.tags.as_ref() %}
                    <footer>
                        <p>{{ tags }}</p>
                    </footer>
                    {% endif %}
                </article>
                {% endfor %}
            </div>
            {% endif %}
        </section>
    </main>
</body>
</html>"#,
    ext = "html"
)]
struct ApplicationsTemplate<'a> {
    apps: &'a [ApplicationView],
    total_apps: usize,
}

async fn root_get() -> impl IntoResponse {
    render_stub("Dashboard", "/")
}

async fn regions_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(r) = ensure_owner(&state, &jar) {
        return r.into_response();
    }
    let (regions, _) = load_regions(&state).await;
    let total_regions = regions.len();
    let premium_count = regions.iter().filter(|r| r.is_premium).count();
    let html = RegionsPageTemplate {
        regions: &regions,
        total_regions,
        premium_count,
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
    let html = ProductsPageTemplate {
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
    let total_images = os_list.len();
    let html = OsCatalogTemplate {
        os_list: &os_list,
        total_images,
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
    let total_apps = apps.len();
    let html = ApplicationsTemplate {
        apps: &apps,
        total_apps,
    }
    .render()
    .unwrap();
    inject_context(&state, &jar, html)
}

// ---------- Instance Detail & Actions ----------
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Instance {{ instance_id }}</title></head>
<body>
    <h1>Instance {{ instance_id }}</h1>
    <pre>{{ instance_json }}</pre>
    <p><a href="/instances">Back</a></p>
</body>
</html>"#,
    ext = "html"
)]
struct InstanceDetailTemplate {
    instance_id: String,
    instance_json: String,
}

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
    let json = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into());
    inject_context(
        &state,
        &jar,
        InstanceDetailTemplate {
            instance_id,
            instance_json: json,
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
#[derive(Template)]
#[template(
    source = r#"<!DOCTYPE html>
<html>
<head><title>Bulk Refund</title></head>
<body>
    <h1>Bulk subscription refund</h1>
    <form method="post">
        <textarea name="ids" rows="6" cols="40"></textarea>
        <button type="submit">Submit</button>
    </form>
</body>
</html>"#,
    ext = "html"
)]
struct BulkRefundTemplate;

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
    Html(BulkRefundTemplate.render().unwrap()).into_response()
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Load environment variables (.env) if present
    let _ = dotenvy::dotenv();

    // Initialize user store from existing users.json if present
    let user_store = {
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

    let api_base_url = std::env::var("API_BASE_URL").unwrap_or_else(|_| "".into());
    let api_token = std::env::var("API_TOKEN").unwrap_or_else(|_| "".into());
    let client = Client::builder().build().unwrap();
    let state = AppState {
        users: user_store,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        api_base_url,
        api_token,
        default_customer_cache: Arc::new(Mutex::new(None)),
        client,
    };

    // Build router with initial endpoints
    let app = Router::new()
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
        // Serve static files (CSS, icons) at /static/*
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:5000".parse().unwrap();
    tracing::info!(%addr, "Starting Zyffiliate Rust server");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
