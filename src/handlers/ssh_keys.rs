use axum::{
    extract::{State, Form, Query},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::Value;

use crate::models::{AppState, SshKeyView};
use crate::templates::SshKeysTemplate;
use crate::handlers::helpers::{
    build_template_globals, current_username_from_jar, ensure_owner,
    fetch_default_customer_id, render_template, TemplateGlobals,
    detail_requires_customer, api_call_wrapper, load_ssh_keys_api,
    plain_html,
};

#[derive(Deserialize)]
pub struct SshKeysForm {
    action: Option<String>,
    name: Option<String>,
    public_key: Option<String>,
    ssh_key_id: Option<String>,
}

pub async fn ssh_keys_get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<HashMap<String, String>>,
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

pub async fn ssh_keys_post(
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
