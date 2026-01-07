use axum::{
    extract::{Form, Path, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;

use crate::models::{AppState, ConfirmationAction};
use crate::templates::{AboutTemplate, ConfirmationTemplate};
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

pub async fn confirmation_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((action_str, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let action = match ConfirmationAction::from_str(&action_str) {
        Some(a) => a,
        None => return Redirect::to("/").into_response(),
    };

    let TemplateGlobals {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
    } = build_template_globals(&state, &jar);

    let mut title = "Confirm Action".to_string();
    let mut message = "Are you sure you want to proceed?".to_string();
    let mut target_url = "/".to_string();
    let mut confirm_label = "Proceed".to_string();
    let mut cancel_url = "/".to_string();
    let mut button_class = "btn-primary".to_string();
    let mut hidden_fields = vec![];

    match action {
        ConfirmationAction::DeleteUser => {
            title = "Delete User".into();
            message = format!("Are you sure you want to permanently delete user '{}'?", id);
            target_url = format!("{}/users/{}/delete", base_url, id);
            confirm_label = "Delete User".into();
            cancel_url = format!("{}/users/{}", base_url, id);
            button_class = "btn-danger".into();
        }
        ConfirmationAction::DeleteInstance => {
            title = "Delete Instance".into();
            message = format!("Are you sure you want to permanently delete instance '{}'?", id);
            target_url = format!("{}/instance/{}/delete", base_url, id);
            confirm_label = "Delete Instance".into();
            cancel_url = format!("{}/instance/{}", base_url, id);
            button_class = "btn-danger".into();
        }
        ConfirmationAction::PowerOnInstance => {
            title = "Power On Instance".into();
            message = format!("Request power on for instance '{}'?", id);
            target_url = format!("{}/instance/{}/poweron", base_url, id);
            confirm_label = "Power On".into();
            cancel_url = format!("{}/instance/{}", base_url, id);
            button_class = "btn-primary".into();
        }
        ConfirmationAction::PowerOffInstance => {
            title = "Power Off Instance".into();
            message = format!("Request power off for instance '{}'?", id);
            target_url = format!("{}/instance/{}/poweroff", base_url, id);
            confirm_label = "Power Off".into();
            cancel_url = format!("{}/instance/{}", base_url, id);
            button_class = "btn-warning".into();
        }
        ConfirmationAction::ResetInstance => {
            title = "Reset Instance".into();
            message = format!("Request immediate reset for instance '{}'?", id);
            target_url = format!("{}/instance/{}/reset", base_url, id);
            confirm_label = "Reset Instance".into();
            cancel_url = format!("{}/instance/{}", base_url, id);
            button_class = "btn-danger".into();
        }
        ConfirmationAction::SwitchVersion => {
            title = "Switch Version".into();
            message = format!("Switch Zy CLI to version '{}'? This will restart the server.", id);
            target_url = format!("{}/about/switch-version", base_url);
            confirm_label = "Switch Now".into();
            cancel_url = format!("{}/about", base_url);
            button_class = "btn-warning".into();
            hidden_fields.push(("version".into(), id.clone()));
        }
        _ => {}
    }

    render_template(&state, &jar, ConfirmationTemplate {
        current_user,
        api_hostname,
        base_url,
        flash_messages,
        has_flash_messages,
        title,
        message,
        target_url,
        confirm_label,
        cancel_url,
        button_class,
        hidden_fields,
    })
}
