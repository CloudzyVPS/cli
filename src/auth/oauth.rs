//! Calls to the token and revocation endpoints.

use serde::Deserialize;

use super::store::StoredProfile;
use super::{now_secs, AuthError};
use crate::config::CLIENT_ID;

#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

impl TokenResponse {
    /// Turn a token response into a stored profile. A response without a
    /// refresh token cannot outlive its access token, which is useless to a
    /// CLI, so it is refused.
    pub fn into_profile(self, url: &str) -> Result<StoredProfile, AuthError> {
        let refresh = self
            .refresh_token
            .filter(|r| !r.is_empty())
            .ok_or_else(|| AuthError::Protocol("the platform issued no refresh token".into()))?;
        Ok(StoredProfile {
            url: url.to_string(),
            access_token: self.access_token,
            refresh_token: refresh,
            expires_at: now_secs() + self.expires_in.unwrap_or(3600),
            scope: self.scope.unwrap_or_default(),
        })
    }
}

/// The outcome of one token-endpoint call: tokens, or the RFC 6749 error.
pub enum TokenOutcome {
    Tokens(TokenResponse),
    Error(ErrorResponse),
}

pub async fn token_request(
    http: &reqwest::Client,
    token_endpoint: &str,
    form: &[(&str, &str)],
) -> Result<TokenOutcome, AuthError> {
    let mut body: Vec<(&str, &str)> = vec![("client_id", CLIENT_ID)];
    body.extend_from_slice(form);
    let resp = http
        .post(token_endpoint)
        .form(&body)
        .send()
        .await
        .map_err(|source| AuthError::Network {
            url: token_endpoint.to_string(),
            source,
        })?;
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|source| AuthError::Network {
        url: token_endpoint.to_string(),
        source,
    })?;
    if status.is_success() {
        return serde_json::from_slice(&bytes)
            .map(TokenOutcome::Tokens)
            .map_err(|e| AuthError::Protocol(format!("unexpected token response: {e}")));
    }
    match serde_json::from_slice::<ErrorResponse>(&bytes) {
        Ok(err) => Ok(TokenOutcome::Error(err)),
        Err(_) => Err(AuthError::Protocol(format!(
            "token endpoint answered {status}"
        ))),
    }
}

/// Exchange a refresh token. `invalid_grant` means the sign-in is over —
/// revoked, expired, replayed, or consent withdrawn.
pub async fn refresh(
    http: &reqwest::Client,
    token_endpoint: &str,
    refresh_token: &str,
) -> Result<TokenResponse, AuthError> {
    match token_request(
        http,
        token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ],
    )
    .await?
    {
        TokenOutcome::Tokens(t) => Ok(t),
        TokenOutcome::Error(e) if e.error == "invalid_grant" => Err(AuthError::SessionExpired),
        TokenOutcome::Error(e) => Err(AuthError::OAuth {
            code: e.error,
            description: e.error_description,
        }),
    }
}

/// RFC 7009 revocation. Revoking the refresh token ends its whole family,
/// including the access token issued with it.
pub async fn revoke(
    http: &reqwest::Client,
    revocation_endpoint: &str,
    token: &str,
) -> Result<(), AuthError> {
    let resp = http
        .post(revocation_endpoint)
        .form(&[
            ("client_id", CLIENT_ID),
            ("token", token),
            ("token_type_hint", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|source| AuthError::Network {
            url: revocation_endpoint.to_string(),
            source,
        })?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(AuthError::Protocol(format!(
            "revocation answered {}",
            resp.status()
        )))
    }
}
