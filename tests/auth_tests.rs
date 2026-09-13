use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use zy::auth::device::{DeviceStart, Poller};
use zy::auth::session::Credential;
use zy::auth::store::{FileStore, StoredProfile};
use zy::auth::{discovery, now_secs, AuthError};
use zy::config::Settings;

async fn provider() -> MockServer {
    let server = MockServer::start().await;
    let base = server.uri();
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": base,
            "authorization_endpoint": format!("{base}/oauth/authorize"),
            "token_endpoint": format!("{base}/oauth/token"),
            "revocation_endpoint": format!("{base}/oauth/revoke"),
            "device_authorization_endpoint": format!("{base}/oauth/device_authorization"),
        })))
        .mount(&server)
        .await;
    server
}

fn settings(url: &str, dir: &std::path::Path) -> Settings {
    Settings {
        url: url.to_string(),
        profile: "default".into(),
        env_token: None,
        config_dir: dir.to_path_buf(),
    }
}

fn tokens(access: &str, refresh: &str) -> serde_json::Value {
    json!({"access_token": access, "refresh_token": refresh, "token_type": "Bearer", "expires_in": 3600, "scope": "openid services.view"})
}

#[tokio::test]
async fn discovery_refuses_a_foreign_issuer() {
    let server = MockServer::start().await;
    Mock::given(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": "https://evil.example",
            "authorization_endpoint": "https://evil.example/a",
            "token_endpoint": "https://evil.example/t",
        })))
        .mount(&server)
        .await;
    let err = discovery::discover(&reqwest::Client::new(), &server.uri())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Protocol(m) if m.contains("issuer mismatch")));
}

#[tokio::test]
async fn device_poll_handles_pending_and_slow_down_then_succeeds() {
    let server = provider().await;
    let oauth_err = |code: &str| ResponseTemplate::new(400).set_body_json(json!({"error": code}));
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(oauth_err("authorization_pending"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(oauth_err("slow_down"))
        .up_to_n_times(1)
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
        ))
        .and(body_string_contains("device_code=czdc_x"))
        .and(body_string_contains("client_id=cloudzy-cli"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens("czat_1", "czrt_1")))
        .with_priority(3)
        .mount(&server)
        .await;

    let start = DeviceStart {
        device_code: "czdc_x".into(),
        user_code: "WDJB-MJHT".into(),
        verification_uri: format!("{}/device", server.uri()),
        verification_uri_complete: None,
        expires_in: 600,
        interval: 1,
    };
    let got = Poller {
        unit: Duration::from_millis(2),
    }
    .poll(
        &reqwest::Client::new(),
        &format!("{}/oauth/token", server.uri()),
        &start,
    )
    .await
    .unwrap();
    assert_eq!(got.access_token, "czat_1");
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path() == "/oauth/token")
            .count(),
        3
    );
}

#[tokio::test]
async fn device_poll_stops_on_denial_and_expiry() {
    for (code, want_denied) in [("access_denied", true), ("expired_token", false)] {
        let server = provider().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": code})))
            .mount(&server)
            .await;
        let start = DeviceStart {
            device_code: "czdc_x".into(),
            user_code: "X".into(),
            verification_uri: "u".into(),
            verification_uri_complete: None,
            expires_in: 600,
            interval: 1,
        };
        let err = Poller {
            unit: Duration::from_millis(1),
        }
        .poll(
            &reqwest::Client::new(),
            &format!("{}/oauth/token", server.uri()),
            &start,
        )
        .await
        .unwrap_err();
        if want_denied {
            assert!(matches!(err, AuthError::Denied), "{code}: {err:?}");
        } else {
            assert!(matches!(err, AuthError::Expired), "{code}: {err:?}");
        }
    }
}

#[tokio::test]
async fn expiring_session_refreshes_and_persists_the_rotated_pair() {
    let server = provider().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=czrt_old"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens("czat_new", "czrt_new")))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::new(dir.path().to_path_buf());
    store
        .save(
            "default",
            &StoredProfile {
                url: server.uri(),
                access_token: "czat_old".into(),
                refresh_token: "czrt_old".into(),
                expires_at: now_secs() + 10,
                scope: String::new(),
            },
        )
        .unwrap();

    let cred =
        Credential::resolve(&settings(&server.uri(), dir.path()), reqwest::Client::new()).unwrap();
    assert_eq!(cred.bearer().await.unwrap(), "czat_new");
    // Second call uses the cached token: the mock expects exactly one refresh.
    assert_eq!(cred.bearer().await.unwrap(), "czat_new");
    let saved = store.load("default").unwrap().unwrap();
    assert_eq!(
        (saved.access_token.as_str(), saved.refresh_token.as_str()),
        ("czat_new", "czrt_new")
    );
    assert!(saved.expires_at > now_secs() + 3000);
}

#[tokio::test]
async fn rejected_token_refreshes_once_then_dead_refresh_signs_out() {
    let server = provider().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::new(dir.path().to_path_buf());
    store
        .save(
            "default",
            &StoredProfile {
                url: server.uri(),
                access_token: "czat_live".into(),
                refresh_token: "czrt_dead".into(),
                expires_at: now_secs() + 3600,
                scope: String::new(),
            },
        )
        .unwrap();

    let cred =
        Credential::resolve(&settings(&server.uri(), dir.path()), reqwest::Client::new()).unwrap();
    assert_eq!(cred.bearer().await.unwrap(), "czat_live");
    let err = cred.after_unauthorized("czat_live").await.unwrap_err();
    assert!(matches!(err, AuthError::SessionExpired));
    assert!(
        store.load("default").unwrap().is_none(),
        "a dead sign-in must be forgotten"
    );
}

#[tokio::test]
async fn stored_sign_in_is_never_sent_to_another_url() {
    let dir = tempfile::tempdir().unwrap();
    FileStore::new(dir.path().to_path_buf())
        .save(
            "default",
            &StoredProfile {
                url: "https://dash.cloudzy.com".into(),
                access_token: "czat_a".into(),
                refresh_token: "czrt_a".into(),
                expires_at: now_secs() + 3600,
                scope: String::new(),
            },
        )
        .unwrap();
    let other = settings("https://attacker.example", dir.path());
    assert!(matches!(
        Credential::resolve(&other, reqwest::Client::new()),
        Err(AuthError::NotSignedIn)
    ));
}

#[tokio::test]
async fn developer_token_takes_precedence_and_is_never_refreshed() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = settings("https://dash.cloudzy.com", dir.path());
    s.env_token = Some("hpt_abc".into());
    let cred = Credential::resolve(&s, reqwest::Client::new()).unwrap();
    assert_eq!(cred.bearer().await.unwrap(), "hpt_abc");
    assert!(cred.after_unauthorized("hpt_abc").await.unwrap().is_none());
}
