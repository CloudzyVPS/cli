//! Device sign-in (RFC 8628) for hosts without a browser.

use std::time::Duration;

use serde::Deserialize;

use super::oauth::{token_request, TokenOutcome, TokenResponse};
use super::AuthError;
use crate::config::CLIENT_ID;

pub const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

pub async fn start(
    http: &reqwest::Client,
    endpoint: &str,
    scope: &str,
) -> Result<DeviceStart, AuthError> {
    let resp = http
        .post(endpoint)
        .form(&[("client_id", CLIENT_ID), ("scope", scope)])
        .send()
        .await
        .map_err(|source| AuthError::Network {
            url: endpoint.to_string(),
            source,
        })?;
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|source| AuthError::Network {
        url: endpoint.to_string(),
        source,
    })?;
    if status.is_success() {
        return serde_json::from_slice(&bytes).map_err(|e| {
            AuthError::Protocol(format!("unexpected device authorization response: {e}"))
        });
    }
    match serde_json::from_slice::<super::oauth::ErrorResponse>(&bytes) {
        Ok(e) => Err(AuthError::OAuth {
            code: e.error,
            description: e.error_description,
        }),
        Err(_) => Err(AuthError::Protocol(format!(
            "device authorization answered {status}"
        ))),
    }
}

/// Polls the token endpoint for one device authorization.
pub struct Poller {
    /// Length of one "second" of the server's interval. Tests shrink it.
    pub unit: Duration,
}

impl Default for Poller {
    fn default() -> Self {
        Poller {
            unit: Duration::from_secs(1),
        }
    }
}

impl Poller {
    pub async fn poll(
        &self,
        http: &reqwest::Client,
        token_endpoint: &str,
        start: &DeviceStart,
    ) -> Result<TokenResponse, AuthError> {
        let deadline = tokio::time::Instant::now() + self.unit * start.expires_in as u32;
        let mut interval = start.interval.max(1);
        loop {
            tokio::time::sleep(self.unit * interval as u32).await;
            if tokio::time::Instant::now() > deadline {
                return Err(AuthError::Expired);
            }
            let outcome = token_request(
                http,
                token_endpoint,
                &[
                    ("grant_type", GRANT_TYPE),
                    ("device_code", &start.device_code),
                ],
            )
            .await?;
            match outcome {
                TokenOutcome::Tokens(t) => return Ok(t),
                TokenOutcome::Error(e) => match e.error.as_str() {
                    "authorization_pending" => {}
                    // §3.5: increase the interval by 5 seconds for this and
                    // all subsequent requests.
                    "slow_down" => interval += 5,
                    "access_denied" => return Err(AuthError::Denied),
                    "expired_token" => return Err(AuthError::Expired),
                    _ => {
                        return Err(AuthError::OAuth {
                            code: e.error,
                            description: e.error_description,
                        })
                    }
                },
            }
        }
    }
}
