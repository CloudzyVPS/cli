//! End-to-end: the real `zy` binary against a mock Cloudzy API.

use std::process::Output;

use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Env {
    server: MockServer,
    config: tempfile::TempDir,
}

impl Env {
    async fn new() -> Env {
        Env {
            server: MockServer::start().await,
            config: tempfile::tempdir().unwrap(),
        }
    }

    async fn zy(&self, args: &[&str], token: Option<&str>) -> Output {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_zy"));
        cmd.args(args)
            .env("CLOUDZY_URL", self.server.uri())
            .env("CLOUDZY_CONFIG_DIR", self.config.path())
            .env_remove("CLOUDZY_PROFILE")
            .env("NO_COLOR", "1")
            .stdin(std::process::Stdio::null());
        match token {
            Some(t) => cmd.env("CLOUDZY_TOKEN", t),
            None => cmd.env_remove("CLOUDZY_TOKEN"),
        };
        tokio::task::spawn_blocking(move || cmd.output().unwrap())
            .await
            .unwrap()
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn server_json() -> Value {
    json!({"id": "3f2a1b4c-0000-4000-8000-00000000abcd", "hostname": "web-1", "state": "active", "powerState": "running",
           "region": "fra", "ipAddress": "203.0.113.7", "cpu": 2, "ramMb": 4096, "diskGb": 80, "plan": {"name": "Standard 4GB"}})
}

#[tokio::test]
async fn servers_list_renders_a_table_and_raw_json() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/services"))
        .and(header("authorization", "Bearer hpt_ci"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([server_json()])))
        .mount(&env.server)
        .await;

    let out = env.zy(&["servers", "list"], Some("hpt_ci")).await;
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("web-1") && text.contains("203.0.113.7") && text.contains("Standard 4GB"),
        "{text}"
    );

    let out = env
        .zy(&["servers", "list", "-o", "json"], Some("hpt_ci"))
        .await;
    let parsed: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(parsed[0]["hostname"], "web-1");
}

#[tokio::test]
async fn create_resolves_plan_slug_and_saved_ssh_key_names() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"plans": [{"id": "plan-uuid-1", "slug": "std-4gb", "name": "Standard 4GB"}], "prices": []})))
        .mount(&env.server).await;
    Mock::given(path("/api/v1/ssh-keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!([{"id": "k1", "name": "laptop", "publicKey": "ssh-ed25519 AAAAC3Nza laptop"}]),
        ))
        .mount(&env.server)
        .await;
    Mock::given(method("POST")).and(path("/api/v1/services"))
        .and(body_json(json!({
            "planId": "plan-uuid-1", "region": "fra", "hostname": "web-1", "osTemplateId": "ubuntu-24.04",
            "sshAuthorizedKeys": ["ssh-ed25519 AAAAC3Nza laptop"],
            "config": {"ocaName": "wordpress", "ocaParams": {"site_title": "Blog"}}
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "new-1", "hostname": "web-1", "state": "provisioning"})))
        .expect(1)
        .mount(&env.server).await;

    Mock::given(method("POST"))
        .and(path("/api/v1/pricing/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"subtotalMonthlyCents": 1500, "grandTotalCents": 1500, "currency": "USD"}),
        ))
        .mount(&env.server)
        .await;

    let out = env
        .zy(
            &[
                "servers",
                "create",
                "--hostname",
                "web-1",
                "--plan",
                "std-4gb",
                "--region",
                "fra",
                "--os",
                "ubuntu-24.04",
                "--ssh-key",
                "laptop",
                "--app",
                "wordpress",
                "--app-param",
                "site_title=Blog",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("provisioning"));
}

#[tokio::test]
async fn delete_without_yes_and_without_a_terminal_refuses() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(server_json()))
        .mount(&env.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&env.server)
        .await;
    let out = env.zy(&["servers", "delete", "s1"], Some("hpt_ci")).await;
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("--yes"));
}

#[tokio::test]
async fn delete_with_yes_and_power_actions() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(server_json()))
        .mount(&env.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&env.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/services/s1/power"))
        .and(body_json(json!({"action": "force-stop"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status": "ok", "action": "force-stop"})),
        )
        .expect(1)
        .mount(&env.server)
        .await;

    assert!(env
        .zy(&["servers", "delete", "s1", "--yes"], Some("hpt_ci"))
        .await
        .status
        .success());
    let out = env
        .zy(&["servers", "power", "s1", "force-stop"], Some("hpt_ci"))
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
}

#[tokio::test]
async fn api_errors_exit_1_with_a_hint() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/services"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({"error": "api token is missing the required scope: services.view", "code": "insufficient_scope"})))
        .mount(&env.server).await;
    let out = env.zy(&["servers", "list"], Some("hpt_narrow")).await;
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("services.view") && err.contains("hint:"),
        "{err}"
    );
}

#[tokio::test]
async fn without_any_credential_it_says_how_to_sign_in() {
    let env = Env::new().await;
    let out = env.zy(&["whoami"], None).await;
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("zy login"));
    assert!(
        env.server.received_requests().await.unwrap().is_empty(),
        "nothing may be sent without a credential"
    );
}

#[tokio::test]
async fn whoami_and_billing_balance() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"id": "u1", "email": "ada@example.test", "name": "Ada", "role": "customer"}),
        ))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/billing/summary"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"balance": 123456, "refundableBalance": 0, "currency": "USD"}),
            ),
        )
        .mount(&env.server)
        .await;
    let out = env.zy(&["whoami"], Some("hpt_ci")).await;
    assert!(
        stdout(&out).contains("ada@example.test"),
        "{}",
        stderr(&out)
    );
    let out = env.zy(&["billing", "balance"], Some("hpt_ci")).await;
    assert!(stdout(&out).contains("1234.56 USD"), "{}", stdout(&out));
}

#[tokio::test]
async fn redirected_output_has_no_ansi_escapes() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(server_json()))
        .mount(&env.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&env.server)
        .await;
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_zy"));
    cmd.args(["servers", "delete", "s1", "--yes"])
        .env("CLOUDZY_URL", env.server.uri())
        .env("CLOUDZY_CONFIG_DIR", env.config.path())
        .env("CLOUDZY_TOKEN", "hpt_ci")
        .env_remove("NO_COLOR");
    let out = tokio::task::spawn_blocking(move || cmd.output().unwrap())
        .await
        .unwrap();
    assert!(out.status.success());
    assert!(
        !stderr(&out).contains('\u{1b}'),
        "piped stderr must not carry colour codes: {:?}",
        stderr(&out)
    );
}

#[tokio::test]
async fn activity_and_usage_use_live_fields_without_changing_json() {
    let env = Env::new().await;
    let activity = json!([{"createdAt":"2026-10-07T15:16:02Z","fromState":"","toState":"provisioning","reason":"service created","actorType":"system","actor":""}]);
    let usage = json!({"cpu":{"available":true,"current":60.43,"limit":100,"unit":"%"},"ram":{"available":true,"current":382,"limit":512,"unit":"MB"},"disk":{"available":false,"current":0,"limit":20,"unit":"GB"}});
    for (route, body) in [("activity", activity.clone()), ("usage", usage.clone())] {
        Mock::given(path(format!("/api/v1/services/s1/{route}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&env.server)
            .await;
    }
    let out = env.zy(&["servers", "activity", "s1"], Some("hpt_ci")).await;
    assert!(out.status.success(), "{}", stderr(&out));
    for expected in [
        "2026-10-07T15:16:02Z",
        "provisioning",
        "service created",
        "system",
    ] {
        assert!(stdout(&out).contains(expected), "{}", stdout(&out));
    }
    let out = env.zy(&["servers", "usage", "s1"], Some("hpt_ci")).await;
    for expected in ["60.43%", "382 / 512 MB", "unavailable"] {
        assert!(stdout(&out).contains(expected), "{}", stdout(&out));
    }
    assert!(!stdout(&out).contains("Disk %"));
    for (command, expected) in [("activity", activity), ("usage", usage)] {
        let out = env
            .zy(&["servers", command, "s1", "-o", "json"], Some("hpt_ci"))
            .await;
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap(),
            expected
        );
    }
}

#[tokio::test]
async fn firewall_aliases_and_restore_confirmation_reach_the_backend() {
    let env = Env::new().await;
    for direction in ["inbound", "outbound"] {
        Mock::given(method("POST")).and(path("/api/v1/services/s1/firewall"))
            .and(body_json(json!({"direction":direction,"protocol":"tcp","port":"8080","source":"192.0.2.0/24","action":"allow"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"rule"})))
            .mount(&env.server).await;
    }
    for alias in [
        None,
        Some("in"),
        Some("inbound"),
        Some("out"),
        Some("outbound"),
    ] {
        let mut args = vec![
            "firewall",
            "add",
            "s1",
            "--port",
            "8080",
            "--source",
            "192.0.2.0/24",
        ];
        if let Some(alias) = alias {
            args.extend(["--direction", alias]);
        }
        let out = env.zy(&args, Some("hpt_ci")).await;
        assert!(out.status.success(), "{}", stderr(&out));
    }
    let bad = env
        .zy(
            &["firewall", "add", "s1", "--direction", "sideways"],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(bad.status.code(), Some(2));
    Mock::given(method("POST"))
        .and(path("/api/v1/services/s1/snapshots/sn1/restore"))
        .and(query_param("confirm", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"state":"restoring"})))
        .expect(1)
        .mount(&env.server)
        .await;
    let refused = env
        .zy(&["snapshots", "restore", "s1", "sn1"], Some("hpt_ci"))
        .await;
    assert_eq!(refused.status.code(), Some(2));
    let out = env
        .zy(
            &["snapshots", "restore", "s1", "sn1", "--yes", "-o", "json"],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["state"],
        "restoring"
    );
}

#[tokio::test]
async fn micro_billing_amounts_keep_precision_and_sign_and_json_stays_raw() {
    let env = Env::new().await;
    let ledger = json!({"data":[{"amountCents":0,"amountMicro":-11086,"description":"hourly"},{"amountCents":0,"amountMicro":1,"description":"refund"},{"amountCents":250,"description":"ordinary"}]});
    let invoices = json!({"data":[{"totalCents":0,"totalMicroCents":11086},{"totalCents":250}]});
    for (route, response) in [
        ("billing/ledger", ledger.clone()),
        ("invoices", invoices.clone()),
    ] {
        Mock::given(path(format!("/api/v1/{route}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&env.server)
            .await;
    }
    for (command, values, raw) in [
        (
            "ledger",
            vec!["-0.011086 USD", "0.000001 USD", "2.50 USD"],
            ledger,
        ),
        ("invoices", vec!["0.011086 USD", "2.50 USD"], invoices),
    ] {
        let out = env.zy(&["billing", command], Some("hpt_ci")).await;
        for value in values {
            assert!(stdout(&out).contains(value), "{}", stdout(&out));
        }
        let out = env
            .zy(&["billing", command, "-o", "json"], Some("hpt_ci"))
            .await;
        assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), raw);
    }
}

#[tokio::test]
async fn unsupported_ip_choices_and_backups_never_mutate() {
    let env = Env::new().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/services/s1/ips"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ipv6Available":false})))
        .mount(&env.server)
        .await;
    for family in ["ipv4", "ipv6"] {
        let out = env
            .zy(&["ips", "add", "s1", "--family", family], Some("hpt_ci"))
            .await;
        assert!(!out.status.success());
        assert!(
            stderr(&out).contains(if family == "ipv4" {
                "reserved-ips create"
            } else {
                "IPv6 is unavailable"
            }),
            "{}",
            stderr(&out)
        );
    }
    let out = env
        .zy(
            &[
                "servers",
                "create",
                "--hostname",
                "test",
                "--plan",
                "p",
                "--region",
                "r",
                "--backups",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("cannot currently be enabled and verified"));
}

#[tokio::test]
async fn plan_labels_are_resolved_only_for_human_output() {
    let env = Env::new().await;
    let raw = json!({"hostname":"my-hostname","plan":{"id":"p1","name":"my-hostname"}});
    Mock::given(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(raw.clone()))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"plans":[{"id":"p1","name":"VPS 1 GB"}]})),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    let out = env.zy(&["servers", "get", "s1"], Some("hpt_ci")).await;
    assert!(stdout(&out).contains("VPS 1 GB"));
    let out = env
        .zy(&["servers", "get", "s1", "-o", "json"], Some("hpt_ci"))
        .await;
    assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), raw);
}

#[tokio::test]
async fn rebuild_problem_errors_are_readable_and_have_recovery_and_reference() {
    let env = Env::new().await;
    let problem = json!({"type":"https://example.test/errors/502","title":"Bad gateway","detail":"The origin failed before returning a response.","status":502,"instance":"req-1"});
    Mock::given(method("POST"))
        .and(path("/api/v1/services/s1/rebuild"))
        .and(body_json(json!({"osTemplate":"debian-12"})))
        .respond_with(
            ResponseTemplate::new(502)
                .insert_header("cf-ray", "ray-123")
                .set_body_json(problem),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &["servers", "rebuild", "s1", "--os", "debian-12", "--yes"],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(out.status.code(), Some(1));
    for expected in [
        "Pending:",
        "Bad gateway: The origin failed",
        "HTTP 502",
        "ray-123",
        "Completion is uncertain",
        "zy servers activity s1",
    ] {
        assert!(stderr(&out).contains(expected), "{}", stderr(&out));
    }
    assert!(!stderr(&out).contains("\"type\""));
    assert!(out.stdout.is_empty());
}

fn pricing_catalog() -> Value {
    json!({"plans":[{"id":"33333333-3333-3333-3333-333333330001","slug":"vps-512","name":"VPS 512 MB","cpuCores":1,"memoryMb":512,"diskGb":20,"bandwidthTb":1}],
        "prices":[{"planId":"33333333-3333-3333-3333-333333330001","regionId":"sg","billingCycle":"hourly","monthlyEquivCents":495,"currency":"USD","inStock":true},
                  {"planId":"33333333-3333-3333-3333-333333330001","regionId":"sg","billingCycle":"monthly","monthlyEquivCents":495,"currency":"USD","inStock":true}]})
}

#[tokio::test]
async fn plan_selection_and_hourly_rates_and_creation_quote_are_consistent() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pricing_catalog()))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/pricing/quote")).and(method("POST"))
        .and(body_json(json!({"planId":"33333333-3333-3333-3333-333333330001","region":"sg","billingCycle":"hourly","quantity":1,"includeIpv4":true,"extras":{}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"subtotalMonthlyCents":745,"grandTotalCents":1,"currency":"USD","appliedFeatures":[{"key":"ipv4_address","monthlyPriceCents":250}]})))
        .expect(1).mount(&env.server).await;
    Mock::given(path("/api/v1/services"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &[
                "plans", "list", "--region", "sg", "--cycle", "hourly", "-o", "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    let selected: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(selected["region"], "sg");
    assert_eq!(selected["billingCycle"], "hourly");
    assert_eq!(selected["plans"][0]["price"]["billingCycle"], "hourly");
    let out = env
        .zy(
            &["plans", "list", "--region", "sg", "--cycle", "hourly"],
            Some("hpt_ci"),
        )
        .await;
    assert!(stdout(&out).contains("/hr"));
    assert!(stderr(&out).contains("exclude configuration extras"));
    let out = env
        .zy(
            &[
                "servers",
                "create",
                "--hostname",
                "preview",
                "--plan",
                "vps-512",
                "--region",
                "sg",
                "--cycle",
                "hourly",
                "--dry-run",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    let quote: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(quote["quote"]["subtotalMonthlyCents"], 745);
    assert_eq!(quote["quote"]["appliedFeatures"][0]["key"], "ipv4_address");
    assert!(!stderr(&out).contains("Pending:"));
}

#[tokio::test]
async fn catalog_out_of_stock_stops_creation_before_mutation() {
    let env = Env::new().await;
    let mut catalog = pricing_catalog();
    catalog["prices"][0]["inStock"] = json!(false);
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalog))
        .mount(&env.server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &[
                "servers",
                "create",
                "--hostname",
                "test",
                "--plan",
                "vps-512",
                "--region",
                "sg",
                "--cycle",
                "hourly",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("out of stock"));
}

#[tokio::test]
async fn reserved_ip_preview_is_read_only_and_purchase_requires_acceptance() {
    let env = Env::new().await;
    let out = env
        .zy(
            &[
                "reserved-ips",
                "create",
                "--region",
                "sg",
                "--count",
                "2",
                "--dry-run",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    let preview: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(preview["totalMonthlyCents"], 500);
    assert_eq!(preview["refundable"], false);
    assert_eq!(preview["autoRenew"], true);
    assert!(preview["priceSource"]
        .as_str()
        .unwrap()
        .contains("not a live server quote"));
    let refused = env
        .zy(
            &["reserved-ips", "create", "--region", "sg"],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(refused.status.code(), Some(2));
    Mock::given(method("POST"))
        .and(path("/api/v1/account/reserved-ips"))
        .and(body_json(json!({"region":"sg","count":1})))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data":[{"id":"ip1","nextBillAt":"2026-11-07","autoRenew":true}]}),
        ))
        .expect(1)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &[
                "reserved-ips",
                "create",
                "--region",
                "sg",
                "--yes",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["data"][0]["nextBillAt"],
        "2026-11-07"
    );
    assert!(stderr(&out).contains("does not confirm the charged amount"));
}

#[tokio::test]
async fn resize_preview_quotes_resources_without_applying_the_change() {
    let env = Env::new().await;
    let source = json!({"id":"s1","planId":"33333333-3333-3333-3333-333333330001","cpu":1,"ramMb":512,"diskGb":20,"bandwidthTb":1,"region":"sg","billingCycle":"hourly","ipAddress":"192.0.2.1","priceMonthly":7.45,"priceHourly":0.0110863095});
    Mock::given(path("/api/v1/services/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(source))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pricing_catalog()))
        .mount(&env.server)
        .await;
    for (cpu, disk, monthly) in [(0, 0, 745), (1, 1, 1155)] {
        Mock::given(method("POST")).and(path("/api/v1/pricing/quote"))
            .and(body_json(json!({"planId":"33333333-3333-3333-3333-333333330001","region":"sg","billingCycle":"hourly","quantity":1,"includeIpv4":true,"extras":{"cpuCores":cpu,"ramGb":0,"diskGb":disk,"bandwidthTb":0}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"subtotalMonthlyCents":monthly,"currency":"USD"})))
            .expect(1).mount(&env.server).await;
    }
    Mock::given(path("/api/v1/services/s1/resize"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &[
                "servers",
                "resize",
                "s1",
                "--cpu",
                "2",
                "--ram-mb",
                "1024",
                "--disk-gb",
                "21",
                "--dry-run",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    let preview: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(preview["configurationQuote"]["subtotalMonthlyCents"], 1155);
    assert_eq!(preview["current"]["priceMonthly"], 7.45);
    assert_eq!(preview["requested"]["diskGb"], 21);
    assert!(preview["immediateCharge"].is_null());
}

#[tokio::test]
async fn snapshot_spawn_supplies_selected_resources_and_waits_for_the_new_id() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/services/source")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"planId":"33333333-3333-3333-3333-333333330001","cpu":2,"ramMb":1024,"diskGb":21,"bandwidthTb":1,"region":"sg","billingCycle":"hourly","ipAddress":"192.0.2.1"}))).mount(&env.server).await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pricing_catalog()))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/pricing/quote"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"subtotalMonthlyCents":755,"currency":"USD"})),
        )
        .mount(&env.server)
        .await;
    Mock::given(method("POST")).and(path("/api/v1/services/source/snapshots/sn1/spawn"))
        .and(body_json(json!({"hostname":"clone","planId":"33333333-3333-3333-3333-333333330001","cpuCores":1,"ramMb":512,"diskGb":21,"transferTb":1,"billingCycle":"hourly"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"newServiceId":"new1","hostname":"clone"})))
        .expect(1).mount(&env.server).await;
    Mock::given(path("/api/v1/services/new1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"id":"new1","state":"active","cpu":1,"ramMb":512,"diskGb":21}),
            ),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    let out = env
        .zy(
            &[
                "snapshots",
                "spawn",
                "source",
                "sn1",
                "--hostname",
                "clone",
                "--plan",
                "vps-512",
                "--yes",
                "--wait",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["id"],
        "new1"
    );
    assert!(stderr(&out).contains("zy servers wait new1"));
}

#[tokio::test]
async fn json_problem_errors_retain_full_payload_and_wait_deadlines_are_bounded() {
    let env = Env::new().await;
    let detail = "Origin could not complete the request. ".repeat(30);
    Mock::given(path("/api/v1/services/s1/rebuild")).respond_with(ResponseTemplate::new(502).set_body_json(json!({"title":"Bad gateway","detail":detail,"type":"https://example.test/problems/gateway","instance":"req-42"}))).mount(&env.server).await;
    let out = env
        .zy(
            &[
                "servers",
                "rebuild",
                "s1",
                "--os",
                "debian-12",
                "--yes",
                "-o",
                "json",
            ],
            Some("hpt_ci"),
        )
        .await;
    assert_eq!(out.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["details"]["detail"], detail);
    assert_eq!(error["details"]["instance"], "req-42");
    assert!(error["recovery"]
        .as_str()
        .unwrap()
        .contains("zy servers get s1"));
    Mock::given(path("/api/v1/services/s2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(3))
                .set_body_json(json!({"id":"s2","state":"provisioning"})),
        )
        .mount(&env.server)
        .await;
    let started = std::time::Instant::now();
    let out = env
        .zy(&["servers", "wait", "s2", "--timeout", "1"], Some("hpt_ci"))
        .await;
    assert_eq!(out.status.code(), Some(1));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert!(stderr(&out).contains("zy servers wait s2"));
}

#[tokio::test]
async fn resize_refuses_when_a_generic_quote_cannot_reproduce_the_current_rate() {
    let env = Env::new().await;
    Mock::given(path("/api/v1/services/s1")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"planId":"33333333-3333-3333-3333-333333330001","cpu":1,"ramMb":512,"diskGb":20,"bandwidthTb":1,"region":"sg","billingCycle":"hourly","ipAddress":"192.0.2.1","priceMonthly":9.99}))).mount(&env.server).await;
    Mock::given(path("/api/v1/pricing/catalog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pricing_catalog()))
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/pricing/quote"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"subtotalMonthlyCents":745,"currency":"USD"})),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    Mock::given(path("/api/v1/services/s1/resize"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&env.server)
        .await;
    let out = env
        .zy(&["servers", "resize", "s1", "--cpu", "2"], Some("hpt_ci"))
        .await;
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not match the current service rate"));
}
