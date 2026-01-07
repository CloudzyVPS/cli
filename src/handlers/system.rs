use axum::{
    extract::State,
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;

use crate::models::AppState;
use crate::templates::AboutTemplate;
use crate::update::{check_for_update, Channel};
use super::helpers::{build_template_globals, render_template, TemplateGlobals};

pub async fn about_get(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);

    // We don't check for update on every GET to avoid rate limiting
    render_template(&state, &jar, AboutTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        version: env!("CARGO_PKG_VERSION"),
        latest_version: None,
    })
}

pub async fn about_check_update(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let mut latest = None;
    match check_for_update(Channel::Stable).await {
        Ok(Some(release)) => {
            latest = Some(release.version.to_string());
        }
        Ok(None) => {
            // Already up to date
            latest = Some(env!("CARGO_PKG_VERSION").to_string());
        }
        Err(e) => {
            tracing::error!(%e, "Failed to check for updates");
        }
    }

    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);

    render_template(&state, &jar, AboutTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        version: env!("CARGO_PKG_VERSION"),
        latest_version: latest,
    })
}

