//! End-to-end: the real `zy` binary against a mock Cloudzy API.

use std::process::Output;

use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, method, path};
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
