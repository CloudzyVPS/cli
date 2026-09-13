//! HTTP client for the Cloudzy API (`<url>/api/v1`).

use std::time::{Duration, Instant};

use reqwest::{Method, StatusCode};
use serde_json::Value;

use super::error::ApiError;
use crate::auth::pkce::random_urlsafe;
use crate::auth::session::Credential;

/// Attempts for a rate-limited request before giving up.
const MAX_RATE_LIMIT_RETRIES: usize = 3;
/// Longest `Retry-After` honoured before surfacing the 429.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

/// One API request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    /// Path under `/api/v1`, starting with `/`.
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
    /// Send an `Idempotency-Key`, so a retried create is not a second create.
    pub idempotent: bool,
}

impl Request {
    pub fn new(method: Method, path: impl Into<String>) -> Request {
        Request {
            method,
            path: path.into(),
            query: Vec::new(),
            body: None,
            idempotent: false,
        }
    }
    pub fn get(path: impl Into<String>) -> Request {
        Request::new(Method::GET, path)
    }
    pub fn post(path: impl Into<String>, body: Value) -> Request {
        Request::new(Method::POST, path).body(body)
    }
    pub fn patch(path: impl Into<String>, body: Value) -> Request {
        Request::new(Method::PATCH, path).body(body)
    }
    pub fn put(path: impl Into<String>, body: Value) -> Request {
        Request::new(Method::PUT, path).body(body)
    }
    pub fn delete(path: impl Into<String>) -> Request {
        Request::new(Method::DELETE, path)
    }
    pub fn body(mut self, body: Value) -> Request {
        self.body = Some(body);
        self
    }
    pub fn query(mut self, key: &str, value: impl ToString) -> Request {
        self.query.push((key.to_string(), value.to_string()));
        self
    }
    pub fn query_opt(self, key: &str, value: Option<impl ToString>) -> Request {
        match value {
            Some(v) => self.query(key, v),
            None => self,
        }
    }
    pub fn idempotent(mut self) -> Request {
        self.idempotent = true;
        self
    }
}

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    credential: Credential,
    debug: bool,
}

impl ApiClient {
    pub fn new(http: reqwest::Client, base_url: &str, credential: Credential) -> ApiClient {
        ApiClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            credential,
            debug: false,
        }
    }

    /// Log each request and response line to stderr. Tokens are never logged.
    pub fn with_debug(mut self, debug: bool) -> ApiClient {
        self.debug = debug;
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn credential(&self) -> &Credential {
        &self.credential
    }

    /// Send a request. A 2xx with a body returns the JSON; 204 returns `Null`.
    pub async fn send(&self, req: Request) -> Result<Value, ApiError> {
        let url = format!("{}/api/v1{}", self.base_url, req.path);
        let idempotency_key = req.idempotent.then(|| random_urlsafe(24));
        let mut token = self.credential.bearer().await?;
        let mut refreshed = false;
        let mut rate_limited = 0;

        loop {
            let mut builder = self
                .http
                .request(req.method.clone(), &url)
                .bearer_auth(&token)
                .header(reqwest::header::ACCEPT, "application/json");
            if !req.query.is_empty() {
                builder = builder.query(&req.query);
            }
            if let Some(body) = &req.body {
                builder = builder.json(body);
            }
            if let Some(key) = &idempotency_key {
                builder = builder.header("Idempotency-Key", key);
            }

            let started = Instant::now();
            if self.debug {
                eprintln!("→ {} {}", req.method, url);
            }
            let resp = builder.send().await.map_err(|source| ApiError::Network {
                url: url.clone(),
                source,
            })?;
            let status = resp.status();
            if self.debug {
                eprintln!(
                    "← {} {} ({} ms)",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or(""),
                    started.elapsed().as_millis()
                );
            }
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok());
            let bytes = resp.bytes().await.map_err(|source| ApiError::Network {
                url: url.clone(),
                source,
            })?;

            if status == StatusCode::UNAUTHORIZED && !refreshed {
                refreshed = true;
                if let Some(fresh) = self.credential.after_unauthorized(&token).await? {
                    token = fresh;
                    continue;
                }
            }
            if status == StatusCode::TOO_MANY_REQUESTS && rate_limited < MAX_RATE_LIMIT_RETRIES {
                let wait = Duration::from_secs(retry_after.unwrap_or(2));
                if wait <= MAX_RETRY_AFTER {
                    rate_limited += 1;
                    if self.debug {
                        eprintln!("  rate limited; retrying in {}s", wait.as_secs());
                    }
                    tokio::time::sleep(wait).await;
                    continue;
                }
            }
            if !status.is_success() {
                return Err(ApiError::from_response(status.as_u16(), &bytes));
            }
            if bytes.is_empty() || status == StatusCode::NO_CONTENT {
                return Ok(Value::Null);
            }
            return serde_json::from_slice(&bytes).map_err(|e| ApiError::Decode {
                url,
                reason: e.to_string(),
            });
        }
    }
}

/// The records in a list response: a bare array, or `{ "data": [...] }`.
pub fn items(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(a) => a.clone(),
        Value::Object(o) => match o.get("data").or_else(|| o.get("items")) {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}
