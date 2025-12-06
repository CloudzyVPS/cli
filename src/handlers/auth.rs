use axum::{
    extract::{Form, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;

use crate::models::AppState;
use crate::services::{verify_password, random_session_id};
use crate::templates::LoginTemplate;

use super::helpers::{build_template_globals, current_username_from_jar, resolve_default_endpoint, TemplateGlobals, render_template};

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
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
    render_template(&state, &jar, LoginTemplate {
            current_user,
            api_hostname,
            base_url: base_url.clone(),
            flash_messages,
            has_flash_messages,
            error: None,
        },
    )
}

pub async fn login_post(
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
    render_template(&state, &jar, LoginTemplate {
            current_user,
            api_hostname,
            base_url,
            flash_messages,
            has_flash_messages,
            error: Some("Invalid credentials".into()),
        },
    )
}

pub async fn logout_post(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(sid) = jar.get("session_id").map(|c| c.value().to_string()) {
        state.sessions.lock().unwrap().remove(&sid);
    }
    let cleared = jar.remove(Cookie::new("session_id", ""));
    (cleared, Redirect::to("/login")).into_response()
}

pub async fn root_get(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(username) = current_username_from_jar(&state, &jar) {
        let target = resolve_default_endpoint(&state, &username);
        return Redirect::to(&target).into_response();
    }
    Redirect::to("/login").into_response()
}
