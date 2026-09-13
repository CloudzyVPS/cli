//! Browser sign-in: authorization code + PKCE to a loopback listener.
//!
//! RFC 8252 §7.3: bind 127.0.0.1 on a port the OS picks, register
//! `http://127.0.0.1:<port>/callback` as the redirect, and accept exactly one
//! well-formed callback carrying our `state`. The platform matches the host and
//! path byte-for-byte and lets only the port vary.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use super::discovery::Metadata;
use super::oauth::{token_request, TokenOutcome, TokenResponse};
use super::{pkce, AuthError};
use crate::config::CLIENT_ID;

/// How long to wait for the person to finish in the browser.
pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Loopback {
    listener: TcpListener,
    port: u16,
}

impl Loopback {
    pub async fn bind() -> Result<Loopback, AuthError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| AuthError::Protocol(format!("cannot listen on 127.0.0.1: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| AuthError::Protocol(e.to_string()))?
            .port();
        Ok(Loopback { listener, port })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    /// Serve callbacks until one carries `expected_state`, then return its code.
    ///
    /// A request with the wrong state is answered with an error page and
    /// otherwise ignored: aborting on it would let any local web page that
    /// guesses the port cancel a sign-in in progress.
    pub async fn wait_for_code(
        self,
        expected_state: &str,
        expected_issuer: &str,
        timeout: Duration,
    ) -> Result<String, AuthError> {
        let fut = async {
            loop {
                let (stream, _) = self
                    .listener
                    .accept()
                    .await
                    .map_err(|e| AuthError::Protocol(format!("loopback accept failed: {e}")))?;
                match handle(stream, expected_state, expected_issuer).await {
                    Callback::Code(code) => return Ok(code),
                    Callback::Error(err) => return Err(err),
                    Callback::Ignored => continue,
                }
            }
        };
        tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| AuthError::Timeout)?
    }
}

enum Callback {
    Code(String),
    Error(AuthError),
    Ignored,
}

async fn handle(mut stream: TcpStream, expected_state: &str, expected_issuer: &str) -> Callback {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
            Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let target = head
        .lines()
        .next()
        .and_then(|l| {
            let mut parts = l.split_whitespace();
            (parts.next() == Some("GET"))
                .then(|| parts.next())
                .flatten()
        })
        .unwrap_or("");
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        respond(&mut stream, 400, "Bad request").await;
        return Callback::Ignored;
    };
    if url.path() != "/callback" {
        respond(&mut stream, 404, "Not found").await;
        return Callback::Ignored;
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    if param("state").as_deref() != Some(expected_state) {
        respond(
            &mut stream,
            400,
            "This sign-in link does not belong to the running zy login. Close this tab.",
        )
        .await;
        return Callback::Ignored;
    }
    // RFC 9207: when the provider names itself, it must be the one we asked.
    if let Some(iss) = param("iss") {
        if iss.trim_end_matches('/') != expected_issuer.trim_end_matches('/') {
            respond(
                &mut stream,
                400,
                "Sign-in response came from an unexpected issuer.",
            )
            .await;
            return Callback::Error(AuthError::Protocol(format!(
                "callback issuer {iss} does not match {expected_issuer}"
            )));
        }
    }
    if let Some(code) = param("error") {
        respond(
            &mut stream,
            200,
            "Sign-in was not completed. You can close this tab and return to the terminal.",
        )
        .await;
        if code == "access_denied" {
            return Callback::Error(AuthError::Denied);
        }
        return Callback::Error(AuthError::OAuth {
            code,
            description: param("error_description"),
        });
    }
    match param("code") {
        Some(code) if !code.is_empty() => {
            respond(
                &mut stream,
                200,
                "Signed in to Cloudzy. You can close this tab and return to the terminal.",
            )
            .await;
            Callback::Code(code)
        }
        _ => {
            respond(&mut stream, 400, "The sign-in response carried no code.").await;
            Callback::Ignored
        }
    }
}

async fn respond(stream: &mut TcpStream, status: u16, message: &str) {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Bad Request",
    };
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>Cloudzy CLI</title>\
         <body style=\"font-family:system-ui;margin:4rem auto;max-width:32rem;text-align:center\">\
         <h1 style=\"font-size:1.25rem\">Cloudzy CLI</h1><p>{}</p></body>",
        html_escape(message)
    );
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The URL the person opens.
pub fn authorize_url(
    meta: &Metadata,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    challenge: &str,
) -> Result<String, AuthError> {
    let mut url = Url::parse(&meta.authorization_endpoint)
        .map_err(|e| AuthError::Protocol(format!("bad authorization endpoint: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", scope)
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

/// Run the whole browser flow. `open` is asked to launch the browser; when it
/// cannot, the URL printed by `announce` is the fallback.
pub async fn login(
    http: &reqwest::Client,
    meta: &Metadata,
    scope: &str,
    open: impl FnOnce(&str) -> bool,
    announce: impl FnOnce(&str, bool),
) -> Result<TokenResponse, AuthError> {
    let loopback = Loopback::bind().await?;
    let redirect_uri = loopback.redirect_uri();
    let verifier = pkce::new_verifier();
    let state = pkce::random_urlsafe(24);
    let url = authorize_url(
        meta,
        &redirect_uri,
        scope,
        &state,
        &pkce::challenge(&verifier),
    )?;
    let opened = open(&url);
    announce(&url, opened);
    let code = loopback
        .wait_for_code(&state, &meta.issuer, CALLBACK_TIMEOUT)
        .await?;
    match token_request(
        http,
        &meta.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &verifier),
        ],
    )
    .await?
    {
        TokenOutcome::Tokens(t) => Ok(t),
        TokenOutcome::Error(e) => Err(AuthError::OAuth {
            code: e.error,
            description: e.error_description,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_ignores_wrong_state_and_accepts_right_one() {
        let lb = Loopback::bind().await.unwrap();
        let redirect = lb.redirect_uri();
        assert!(redirect.starts_with("http://127.0.0.1:") && redirect.ends_with("/callback"));
        let waiter = tokio::spawn(async move {
            lb.wait_for_code("good", "https://dash.example.test", Duration::from_secs(10))
                .await
        });

        let http = reqwest::Client::new();
        let r = http
            .get(format!("{redirect}?code=stolen&state=bad"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);
        let r = http
            .get(redirect.replace("/callback", "/favicon.ico"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404);
        let r = http
            .get(format!(
                "{redirect}?code=czac_ok&state=good&iss=https%3A%2F%2Fdash.example.test"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(waiter.await.unwrap().unwrap(), "czac_ok");
    }

    #[tokio::test]
    async fn loopback_reports_denial_and_issuer_mismatch() {
        let lb = Loopback::bind().await.unwrap();
        let redirect = lb.redirect_uri();
        let waiter = tokio::spawn(async move {
            lb.wait_for_code("s", "https://dash.example.test", Duration::from_secs(10))
                .await
        });
        reqwest::get(format!("{redirect}?error=access_denied&state=s"))
            .await
            .unwrap();
        assert!(matches!(waiter.await.unwrap(), Err(AuthError::Denied)));

        let lb = Loopback::bind().await.unwrap();
        let redirect = lb.redirect_uri();
        let waiter = tokio::spawn(async move {
            lb.wait_for_code("s", "https://dash.example.test", Duration::from_secs(10))
                .await
        });
        reqwest::get(format!(
            "{redirect}?code=x&state=s&iss=https%3A%2F%2Fevil.test"
        ))
        .await
        .unwrap();
        assert!(matches!(waiter.await.unwrap(), Err(AuthError::Protocol(_))));
    }

    #[tokio::test]
    async fn loopback_times_out() {
        let lb = Loopback::bind().await.unwrap();
        let err = lb
            .wait_for_code("s", "https://x", Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Timeout));
    }

    #[test]
    fn authorize_url_carries_pkce_and_client() {
        let meta = Metadata {
            issuer: "https://dash.example.test".into(),
            authorization_endpoint: "https://dash.example.test/oauth/authorize".into(),
            token_endpoint: "https://dash.example.test/oauth/token".into(),
            revocation_endpoint: None,
            device_authorization_endpoint: None,
        };
        let u = Url::parse(
            &authorize_url(
                &meta,
                "http://127.0.0.1:5555/callback",
                "openid services.view",
                "st",
                "ch",
            )
            .unwrap(),
        )
        .unwrap();
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "cloudzy-cli");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:5555/callback");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["scope"], "openid services.view");
        assert_eq!(q["state"], "st");
    }
}
