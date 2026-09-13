use serde_json::{json, Value};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use zy::api::ApiClient;
use zy::auth::session::Credential;
use zy::mcp::server::Server;

fn server_for(mock: &MockServer) -> Server {
    Server::new(Ok(ApiClient::new(
        reqwest::Client::new(),
        &mock.uri(),
        Credential::ApiToken("hpt_mcp".into()),
    )))
}

async fn call(s: &Server, name: &str, args: Value) -> Value {
    let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}});
    s.handle(&msg.to_string()).await.unwrap()["result"].clone()
}

#[tokio::test]
async fn list_servers_returns_text_and_structured_content() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/services"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"id": "s1", "hostname": "web-1"}])),
        )
        .mount(&mock)
        .await;
    let r = call(&server_for(&mock), "list_servers", json!({})).await;
    assert_eq!(r["isError"], false);
    assert_eq!(r["structuredContent"]["items"][0]["hostname"], "web-1");
    assert!(r["content"][0]["text"].as_str().unwrap().contains("web-1"));
}

#[tokio::test]
async fn power_and_delete_hit_the_right_endpoints() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/services/s1/power"))
        .and(body_json(json!({"action": "reboot"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;
    let s = server_for(&mock);
    assert_eq!(
        call(&s, "power_server", json!({"id": "s1", "action": "reboot"})).await["isError"],
        false
    );
    let r = call(&s, "delete_server", json!({"id": "s1"})).await;
    assert_eq!(r["structuredContent"]["ok"], true);
}

#[tokio::test]
async fn bad_arguments_and_api_failures_are_tool_errors() {
    let mock = MockServer::start().await;
    Mock::given(path("/api/v1/services/s1/resize"))
        .respond_with(
            ResponseTemplate::new(402).set_body_json(json!({"error": "insufficient balance"})),
        )
        .mount(&mock)
        .await;
    let s = server_for(&mock);

    let r = call(&s, "power_server", json!({"id": "s1", "action": "explode"})).await;
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("must be one of"));

    let r = call(&s, "resize_server", json!({"id": "s1"})).await;
    assert_eq!(r["isError"], true);

    let r = call(&s, "resize_server", json!({"id": "s1", "cpu": 4})).await;
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["status"], 402);
    assert!(r["structuredContent"]["hint"]
        .as_str()
        .unwrap()
        .contains("balance"));
}

#[tokio::test]
async fn list_plans_joins_the_regional_price() {
    let mock = MockServer::start().await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plans": [{"id": "p1", "slug": "std"}],
            "prices": [
                {"planId": "p1", "regionId": "", "billingCycle": "monthly", "monthlyEquivCents": 1000},
                {"planId": "p1", "regionId": "fra", "billingCycle": "monthly", "monthlyEquivCents": 1200},
            ]
        })))
        .mount(&mock)
        .await;
    let s = server_for(&mock);
    let r = call(&s, "list_plans", json!({"region": "fra"})).await;
    assert_eq!(
        r["structuredContent"]["plans"][0]["price"]["monthlyEquivCents"],
        1200
    );
    let r = call(&s, "list_plans", json!({})).await;
    assert_eq!(
        r["structuredContent"]["plans"][0]["price"]["monthlyEquivCents"],
        1000
    );
}
