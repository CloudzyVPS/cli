//! Signing in to the Cloudzy platform.
//!
//! Three ways to hold a credential, in the order they are used:
//!   * `CLOUDZY_TOKEN` — a developer API token (`hpt_…`) from the panel's
//!     Developer API page. For CI and unattended automation; never refreshed.
//!   * `zy login` — OAuth 2.0 authorization code + PKCE (S256) through the
//!     browser, delivered to a one-shot listener on 127.0.0.1 (RFC 8252).
//!   * `zy login --device` — the OAuth device authorization grant (RFC 8628)
//!     for hosts without a browser: approve a short code on any device.
//!
//! Both OAuth flows end in a short-lived access token plus a rotating refresh
//! token, stored per profile by [`store::FileStore`] and refreshed
//! automatically by [`session::Session`].

pub mod browser;
pub mod device;
pub mod discovery;
pub mod oauth;
pub mod pkce;
pub mod session;
pub mod store;

use thiserror::Error;

/// Scopes zy asks for. The developer-API scopes are what the Cloudzy API
/// enforces; the person approves them once on the consent screen.
pub const SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "services.view",
    "services.create",
    "services.manage",
    "services.delete",
    "networking.view",
    "networking.manage",
    "backups.view",
    "backups.manage",
    "billing.view",
    "account.view",
];

pub fn scope_string() -> String {
    SCOPES.join(" ")
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("not signed in — run `zy login`, or set CLOUDZY_TOKEN to a developer API token")]
    NotSignedIn,
    #[error("your sign-in has expired or was revoked — run `zy login` again")]
    SessionExpired,
    #[error("could not reach {url}: {source}")]
    Network { url: String, source: reqwest::Error },
    #[error("{0}")]
    Protocol(String),
    #[error("sign-in failed: {code}{}", description.as_deref().map(|d| format!(" — {d}")).unwrap_or_default())]
    OAuth {
        code: String,
        description: Option<String>,
    },
    #[error("sign-in was denied")]
    Denied,
    #[error("the code expired before it was approved — run `zy login --device` again")]
    Expired,
    #[error("timed out waiting for the browser sign-in to finish")]
    Timeout,
    #[error("credential store: {0}")]
    Store(String),
}

/// Current Unix time in seconds.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
