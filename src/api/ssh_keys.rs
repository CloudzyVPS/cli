use crate::models::SshKeyView;
use super::client::api_call;
use serde_json::Value;

/// Paginated result structure for SSH keys
#[derive(Clone, Debug)]
pub struct PaginatedSshKeys {
    pub ssh_keys: Vec<SshKeyView>,
    pub total_count: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

/// Load SSH keys for the authenticated user (or specific customer if provided).
pub async fn load_ssh_keys(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    customer_id: Option<String>,
) -> Vec<SshKeyView> {
    let mut params = match customer_id {
        Some(cid) => vec![("customerId".to_string(), cid)],
        None => vec![],
    };
    params.push(("limit".to_string(), "100".to_string()));
    let payload = api_call(client, api_base_url, api_token, "GET", "/v1/ssh-keys", None, Some(params)).await;
    
    // Debug logging
    tracing::info!(?payload, "SSH Keys API Response");

    if payload.get("code").and_then(|c| c.as_str()) != Some("OKAY") {
        tracing::error!(?payload, "SSH Keys API returned error");
        return vec![];
    }

    let data = payload.get("data").cloned().unwrap_or(Value::Null);
    let candidates: Vec<Value> = if let Some(arr) = data.as_array() {
        arr.clone()
    } else {
        // Fallback for different API response structures if any
        if let Some(obj) = data.as_object() {
            if let Some(arr) = obj.get("sshKeys").and_then(|v| v.as_array()) {
                arr.clone()
            } else if let Some(arr) = obj.get("ssh_keys").and_then(|v| v.as_array()) {
                arr.clone()
            } else if let Some(arr) = obj.get("keys").and_then(|v| v.as_array()) {
                arr.clone()
            } else {
                vec![]
            }
        } else {
            vec![]
        }
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
            let cid = obj
                .get("customerId")
                .or_else(|| obj.get("customer_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            out.push(SshKeyView {
                id,
                name,
                fingerprint,
                public_key,
                customer_id: cid,
            });
        }
    }
    out
}

/// Load SSH keys with pagination support.
/// Since the API doesn't support server-side pagination for SSH keys yet,
/// this performs client-side pagination.
pub async fn load_ssh_keys_paginated(
    client: &reqwest::Client,
    api_base_url: &str,
    api_token: &str,
    customer_id: Option<String>,
    page: usize,
    per_page: usize,
) -> PaginatedSshKeys {
    let all_keys = load_ssh_keys(client, api_base_url, api_token, customer_id).await;
    let total_count = all_keys.len();
    
    if page == 0 || per_page == 0 {
        return PaginatedSshKeys {
            ssh_keys: all_keys,
            total_count,
            current_page: 0,
            total_pages: 1,
            per_page: total_count,
        };
    }
    
    let total_pages = if total_count == 0 {
        1
    } else {
        (total_count + per_page - 1) / per_page
    };
    
    let current_page = page.max(1).min(total_pages);
    let start_idx = (current_page - 1) * per_page;
    let end_idx = (start_idx + per_page).min(total_count);
    
    let paginated_keys = if start_idx < total_count {
        all_keys[start_idx..end_idx].to_vec()
    } else {
        vec![]
    };
    
    PaginatedSshKeys {
        ssh_keys: paginated_keys,
        total_count,
        current_page,
        total_pages,
        per_page,
    }
}
