use axum::{
    extract::{Form, State},
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;

use crate::models::AppState;
use crate::templates::AboutTemplate;
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
        all_releases: vec![],
    })
}

pub async fn about_check_update(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    let mut latest = None;
    let mut all_releases = vec![];
    
    let client = crate::update::GitHubClient::new(
        crate::update::REPO_OWNER.to_string(),
        crate::update::REPO_NAME.to_string()
    );

    match client.get_all_releases().await {
        Ok(releases) => {
            all_releases = releases;
            if let Some(first) = all_releases.first() {
                latest = Some(first.version.to_string());
            }
        }
        Err(e) => {
            tracing::error!(%e, "Failed to fetch releases");
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
        all_releases,
    })
}

#[derive(serde::Deserialize)]
pub struct SwitchVersionForm {
    pub version: String,
}

pub async fn about_switch_version(
    State(_state): State<AppState>,
    _jar: CookieJar,
    Form(form): Form<SwitchVersionForm>,
) -> impl IntoResponse {
    // Phase 2: Implementation of version switching/self-update
    // For now, we just redirect back with a message that it's coming soon
    tracing::info!("User requested switch to version: {}", form.version);
    println!("User requested switch to version: {}", form.version);
    
    // In a real implementation, this would trigger the background update process
    // and potentially restart the server.
    
    axum::response::Redirect::to("/about")
}

