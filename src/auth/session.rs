//! The credential an API request is made with.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::store::{FileStore, StoredProfile};
use super::{discovery, now_secs, oauth, AuthError};
use crate::config::Settings;

/// Refresh this many seconds before the access token actually expires, so a
/// request never leaves with a token that dies in flight.
const REFRESH_SKEW: u64 = 60;

/// What requests are authenticated with.
#[derive(Clone)]
pub enum Credential {
    /// A developer API token from `CLOUDZY_TOKEN`. Used as-is.
    ApiToken(String),
    /// A stored OAuth sign-in, refreshed as needed.
    Session(Arc<Session>),
}

impl Credential {
    /// Pick the credential for these settings.
    ///
    /// Stored tokens are only ever sent to the URL that issued them: pointing
    /// zy at a different platform with `--url` or `CLOUDZY_URL` does not carry
    /// a sign-in across to it.
    pub fn resolve(settings: &Settings, http: reqwest::Client) -> Result<Credential, AuthError> {
        if let Some(token) = &settings.env_token {
            return Ok(Credential::ApiToken(token.clone()));
        }
        let store = FileStore::new(settings.config_dir.clone());
        match store.load(&settings.profile)? {
            Some(p) if p.url == settings.url => Ok(Credential::Session(Arc::new(Session::new(
                http,
                store,
                settings.profile.clone(),
                p,
            )))),
            _ => Err(AuthError::NotSignedIn),
        }
    }

    /// The bearer token for the next request.
    pub async fn bearer(&self) -> Result<String, AuthError> {
        match self {
            Credential::ApiToken(t) => Ok(t.clone()),
            Credential::Session(s) => s.access_token().await,
        }
    }

    /// Called after a 401. Returns a fresh token worth one retry, or `None`
    /// when retrying cannot help.
    pub async fn after_unauthorized(&self, rejected: &str) -> Result<Option<String>, AuthError> {
        match self {
            Credential::ApiToken(_) => Ok(None),
            Credential::Session(s) => s.refresh_after_rejection(rejected).await.map(Some),
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            Credential::ApiToken(_) => "developer API token (CLOUDZY_TOKEN)",
            Credential::Session(_) => "browser/device sign-in",
        }
    }
}

pub struct Session {
    http: reqwest::Client,
    store: FileStore,
    profile: String,
    state: Mutex<StoredProfile>,
}

impl Session {
    pub fn new(
        http: reqwest::Client,
        store: FileStore,
        profile: String,
        stored: StoredProfile,
    ) -> Session {
        Session {
            http,
            store,
            profile,
            state: Mutex::new(stored),
        }
    }

    pub async fn snapshot(&self) -> StoredProfile {
        self.state.lock().await.clone()
    }

    pub async fn access_token(&self) -> Result<String, AuthError> {
        let mut state = self.state.lock().await;
        if state.expires_at > now_secs() + REFRESH_SKEW {
            return Ok(state.access_token.clone());
        }
        self.rotate(&mut state).await
    }

    async fn refresh_after_rejection(&self, rejected: &str) -> Result<String, AuthError> {
        let mut state = self.state.lock().await;
        // Another request already rotated while this one was in flight.
        if state.access_token != rejected {
            return Ok(state.access_token.clone());
        }
        self.rotate(&mut state).await
    }

    async fn rotate(&self, state: &mut StoredProfile) -> Result<String, AuthError> {
        let meta = discovery::discover(&self.http, &state.url).await?;
        let refreshed =
            match oauth::refresh(&self.http, &meta.token_endpoint, &state.refresh_token).await {
                Ok(t) => t,
                Err(AuthError::SessionExpired) => {
                    // The refresh token is dead for good; keeping it would only
                    // produce the same failure on every command.
                    let _ = self.store.remove(&self.profile);
                    return Err(AuthError::SessionExpired);
                }
                Err(e) => return Err(e),
            };
        *state = refreshed.into_profile(&state.url)?;
        // Refresh tokens rotate: the old one is spent, so the new pair must be
        // persisted before it is used or the next command is signed out.
        self.store.save(&self.profile, state)?;
        Ok(state.access_token.clone())
    }
}
