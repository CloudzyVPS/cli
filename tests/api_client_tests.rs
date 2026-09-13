use serde_json::json;
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use zy::api::{items, ApiClient, ApiError, Request};
use zy::auth::now_secs;
use zy::auth::session::Credential;
use zy::auth::store::{FileStore, StoredProfile};
use zy::config::Settings;

fn token_client(server: &MockServer) -> ApiClient {
    ApiClient::new(
        reqwest::Client::new(),
        &server.uri(),
        Credential::ApiToken("hpt_test".into()),
    )
}

#[tokio::test]
async fn sends_bearer_json_and_query_under_api_v1() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v1/services/abc"))
        .and(header("authorization", "Bearer hpt_test"))
        .and(query_param("dryRun", "1"))
        .and(body_json(json!({"hostname": "web-1"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "abc", "hostname": "web-1"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let got = token_client(&server)
        .send(Request::patch("/services/abc", json!({"hostname": "web-1"})).query("dryRun", 1))
        .await
        .unwrap();
    assert_eq!(got["hostname"], "web-1");
}

#[tokio::test]
async fn no_content_is_null_and_creates_carry_an_idempotency_key() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/ssh-keys/k1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/services"))
        .and(header_exists("idempotency-key"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "new"})))
        .expect(1)
        .mount(&server)
        .await;
    let c = token_client(&server);
    assert!(c
        .send(Request::delete("/ssh-keys/k1"))
        .await
        .unwrap()
        .is_null());
    assert_eq!(
        c.send(Request::post("/services", json!({})).idempotent())
            .await
            .unwrap()["id"],
        "new"
    );
}

#[tokio::test]
async fn platform_errors_become_api_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/services/x"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({"error": "api token is missing the required scope: services.delete", "code": "insufficient_scope"})))
        .mount(&server).await;
    let err = token_client(&server)
        .send(Request::delete("/services/x"))
        .await
        .unwrap_err();
    match &err {
        ApiError::Http { status, code, .. } => {
            assert_eq!(*status, 403);
            assert_eq!(code.as_deref(), Some("insufficient_scope"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn rate_limit_is_retried_after_the_advertised_delay() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/regions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_json(json!({"error": "slow down", "code": "rate_limited"})),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/api/v1/regions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": "us-east"}])))
        .with_priority(2)
        .mount(&server)
        .await;
    let got = token_client(&server)
        .send(Request::get("/regions"))
        .await
        .unwrap();
    assert_eq!(items(&got).len(), 1);
}

#[tokio::test]
async fn developer_token_401_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/services"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"invalid token"}"#))
        .expect(1)
        .mount(&server)
        .await;
    let err = token_client(&server)
        .send(Request::get("/services"))
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(401));
}

#[tokio::test]
async fn session_401_refreshes_once_and_retries_with_the_new_token() {
    let server = MockServer::start().await;
    let base = server.uri();
    Mock::given(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": base, "authorization_endpoint": format!("{base}/oauth/authorize"),
            "token_endpoint": format!("{base}/oauth/token"),
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "czat_new", "refresh_token": "czrt_new", "expires_in": 3600}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/api/v1/auth/me"))
        .and(header("authorization", "Bearer czat_old"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"invalid token"}"#))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/api/v1/auth/me"))
        .and(header("authorization", "Bearer czat_new"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"email": "ada@example.test"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    FileStore::new(dir.path().to_path_buf())
        .save(
            "default",
            &StoredProfile {
                url: base.clone(),
                access_token: "czat_old".into(),
                refresh_token: "czrt_old".into(),
                expires_at: now_secs() + 3600,
                scope: String::new(),
            },
        )
        .unwrap();
    let settings = Settings {
        url: base.clone(),
        profile: "default".into(),
        env_token: None,
        config_dir: dir.path().to_path_buf(),
    };
    let cred = Credential::resolve(&settings, reqwest::Client::new()).unwrap();
    let got = ApiClient::new(reqwest::Client::new(), &base, cred)
        .send(Request::get("/auth/me"))
        .await
        .unwrap();
    assert_eq!(got["email"], "ada@example.test");
}

#[test]
fn items_accepts_bare_arrays_and_data_envelopes() {
    assert_eq!(items(&json!([1, 2])).len(), 2);
    assert_eq!(items(&json!({"data": [1]})).len(), 1);
    assert!(items(&json!({"id": 1})).is_empty());
}
