//! OpenID Provider metadata (`/.well-known/openid-configuration`).

use serde::Deserialize;

use super::AuthError;

#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub device_authorization_endpoint: Option<String>,
}

/// Fetch and check the provider metadata for `base_url`.
///
/// The `issuer` must equal the URL zy was pointed at. Anything else means the
/// document came from somewhere other than the platform the tokens are meant
/// for, and every endpoint in it is untrusted.
pub async fn discover(http: &reqwest::Client, base_url: &str) -> Result<Metadata, AuthError> {
    let url = format!("{base_url}/.well-known/openid-configuration");
    let resp = http
        .get(&url)
        .send()
        .await
        .map_err(|source| AuthError::Network {
            url: url.clone(),
            source,
        })?;
    if !resp.status().is_success() {
        return Err(AuthError::Protocol(format!(
            "{url} answered {} — is {base_url} a Cloudzy platform URL?",
            resp.status()
        )));
    }
    let meta: Metadata = resp
        .json()
        .await
        .map_err(|e| AuthError::Protocol(format!("{url} did not return provider metadata: {e}")))?;
    if meta.issuer.trim_end_matches('/') != base_url.trim_end_matches('/') {
        return Err(AuthError::Protocol(format!(
            "issuer mismatch: {base_url} advertises issuer {} — refusing to sign in",
            meta.issuer
        )));
    }
    Ok(meta)
}
