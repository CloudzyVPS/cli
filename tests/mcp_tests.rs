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

#[tokio::test]
async fn plan_selector_errors_and_unavailable_prices_are_shared_with_cli() {
    let mock = MockServer::start().await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plans":[{"id":"p1"}], "regions":[{"id":"sg"}],
            "prices":[{"planId":"p1","regionId":"","billingCycle":"monthly","monthlyEquivCents":1000}]
        }))).mount(&mock).await;
    let s = server_for(&mock);
    for (args, expected) in [
        (json!({"region":"typo"}), "zy regions list"),
        (json!({"billingCycle":"typo"}), "supported cycles"),
    ] {
        let result = call(&s, "list_plans", args).await;
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(expected));
    }
    let result = call(&s, "list_plans", json!({"region":"sg"})).await;
    assert_eq!(result["isError"], false);
    assert!(result["structuredContent"]["plans"][0]["priceScope"]
        .as_str()
        .unwrap()
        .contains("base price fallback"));
    let result = call(
        &s,
        "list_plans",
        json!({"region":"sg","billingCycle":"weekly"}),
    )
    .await;
    assert_eq!(result["isError"], false);
    assert!(result["structuredContent"]["plans"][0]["price"].is_null());
}

#[tokio::test]
async fn mcp_quotes_use_only_explicit_route_absence_compatibility() {
    let mock = MockServer::start().await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plans":[{"id":"p1","slug":"std","cpuCores":1,"memoryMb":512,"diskGb":20}],
            "regions":[{"id":"sg"}],"prices":[]
        })))
        .mount(&mock)
        .await;
    for route in [
        "/api/v1/pricing/quote",
        "/api/v1/plan-capacity",
        "/api/v1/account/reserved-ips/quote",
    ] {
        Mock::given(path(route))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("cf-ray", "example-DFW")
                    .set_body_json(json!({"code":"not_found","error":"no such API route"})),
            )
            .mount(&mock)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/api/v1/pricing/quote/public"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"subtotalMonthlyCents":745,"currency":"USD"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    let s = server_for(&mock);
    let result = call(
        &s,
        "quote_server_configuration",
        json!({"plan":"std","region":"sg","billingCycle":"hourly"}),
    )
    .await;
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(
        result["structuredContent"]["quote"]["subtotalMonthlyCents"],
        745
    );
    assert!(result["structuredContent"]["availability"]["available"].is_null());
    let result = call(&s, "quote_reserved_ips", json!({"region":"sg"})).await;
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(result["structuredContent"]["advisory"], true);
    assert_eq!(result["structuredContent"]["totalMonthlyCents"], 250);
    assert!(mock
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|r| r.method == "GET" || r.url.path().contains("/pricing/quote")));
}

#[tokio::test]
async fn firewall_and_restore_use_the_same_contract_as_the_cli() {
    use wiremock::matchers::query_param;
    let mock = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/v1/services/s1/firewall"))
        .and(body_json(json!({"direction":"inbound","protocol":"tcp","port":"8080","source":"0.0.0.0/0","action":"allow"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"rule"})))
        .expect(1).mount(&mock).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/services/s1/snapshots/sn1/restore"))
        .and(query_param("confirm", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"state":"restoring"})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server_for(&mock);
    let result = call(
        &server,
        "add_firewall_rule",
        json!({"server":"s1","direction":"in","protocol":"tcp","port":"8080","action":"allow"}),
    )
    .await;
    assert_eq!(result["isError"], false);
    let result = call(
        &server,
        "restore_snapshot",
        json!({"server":"s1","snapshot":"sn1"}),
    )
    .await;
    assert_eq!(result["isError"], false);
    let result = call(
        &server,
        "attach_server_ip",
        json!({"server":"s1","family":"ipv4"}),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("reserved-ips create"));
}

#[tokio::test]
async fn reserved_ip_mcp_quote_and_purchase_share_price_guards() {
    let mock = MockServer::start().await;
    Mock::given(method("GET")).and(path("/api/v1/account/reserved-ips/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"region":"sg","family":"ipv4","count":1,
        "unitMonthlyCents":300,"totalMonthlyCents":300,"currency":"USD","billingCycle":"monthly","autoRenew":true,"refundable":false}))).mount(&mock).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/account/reserved-ips"))
        .and(body_json(
            json!({"region":"sg","count":1,"expectedTotalCents":300,"expectedCurrency":"USD"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data":[],"receipt":{"chargeStatus":"pending","chargedAmountCents":null}}),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    let s = server_for(&mock);
    let q = call(&s, "quote_reserved_ips", json!({"region":"sg"})).await;
    assert_eq!(q["isError"], false);
    assert_eq!(q["structuredContent"]["totalMonthlyCents"], 300);
    let r = call(&s, "reserve_ips", json!({"region":"sg"})).await;
    assert_eq!(r["isError"], false);
    assert!(r["structuredContent"]["receipt"]["chargedAmountCents"].is_null());
}
