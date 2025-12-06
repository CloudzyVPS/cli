use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::http::StatusCode;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::models::{AppState, CurrentUser};

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

pub fn inject_context(state: &AppState, jar: &CookieJar, mut html: String) -> Response {
    // Inject a global context object into the HTML.
    // We don't use this currently but it's for potential JS needs.
    let api_hostname = crate::utils::hostname_from_url(&state.api_base_url);
    let base_url = state.public_base_url.clone();
    let current_user = build_current_user(state, jar);
    let context = serde_json::json!({
        "apiHostname": api_hostname,
        "baseUrl": base_url,
        "currentUser": current_user,
    });
    let context_str = serde_json::to_string(&context).unwrap();
    let inject = format!(
        r#"<script>window.__APP_CONTEXT__ = {};</script></body>"#,
        context_str
    );
    html = html.replace("</body>", &inject);
    Html(html).into_response()
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

pub fn render_template<T: askama::Template>(state: &AppState, jar: &CookieJar, t: T) -> Response {
    match t.render() {
        Ok(body) => inject_context(state, jar, body),
        Err(e) => {
            tracing::error!(%e, "Template render error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}
