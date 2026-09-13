//! Every Cloudzy API operation zy uses, as a request builder.
//!
//! The CLI commands and the MCP tools both go through these, so the two can
//! never disagree about a path or a body shape. Paths are under `/api/v1`.

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::Request;

fn seg(id: &str) -> String {
    url::form_urlencoded::byte_serialize(id.as_bytes()).collect()
}

/// Drop `null` members so an unset option is omitted, not sent as null.
fn compact(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, v)| !v.is_null())
                .collect::<Map<_, _>>(),
        ),
        other => other,
    }
}

// ─── Account ────────────────────────────────────────────────────────────

pub fn whoami() -> Request {
    Request::get("/auth/me")
}
pub fn balance() -> Request {
    Request::get("/billing/summary")
}
pub fn ledger(page: u32, per_page: u32) -> Request {
    Request::get("/billing/ledger")
        .query("page", page)
        .query("perPage", per_page)
}
pub fn invoices(page: u32, per_page: u32) -> Request {
    Request::get("/invoices")
        .query("page", page)
        .query("perPage", per_page)
}
pub fn activity() -> Request {
    Request::get("/account/activity")
}

// ─── Catalog ────────────────────────────────────────────────────────────

pub fn regions() -> Request {
    Request::get("/regions")
}
pub fn pricing_catalog() -> Request {
    Request::get("/pricing/catalog")
}
pub fn os_templates() -> Request {
    Request::get("/os-templates")
}
pub fn apps() -> Request {
    Request::get("/ocas")
}
pub fn app(name: &str) -> Request {
    Request::get(format!("/ocas/{}", seg(name)))
}

// ─── Servers ────────────────────────────────────────────────────────────

pub fn servers() -> Request {
    Request::get("/services")
}
pub fn server(id: &str) -> Request {
    Request::get(format!("/services/{}", seg(id)))
}
pub fn server_status(id: &str) -> Request {
    Request::get(format!("/services/{}/status", seg(id)))
}
pub fn server_usage(id: &str) -> Request {
    Request::get(format!("/services/{}/usage", seg(id)))
}
pub fn server_activity(id: &str) -> Request {
    Request::get(format!("/services/{}/activity", seg(id)))
}

/// Body of `POST /services`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateServer {
    pub plan_id: String,
    pub region: String,
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ssh_authorized_keys: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reserved_ip_ids: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_backups: bool,
    /// `ocaName`, `ocaParams`, `userData` …
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub config: Map<String, Value>,
}

pub fn create_server(body: &CreateServer) -> Request {
    Request::post(
        "/services",
        serde_json::to_value(body).expect("create body"),
    )
    .idempotent()
}
pub fn delete_server(id: &str) -> Request {
    Request::delete(format!("/services/{}", seg(id)))
}
/// `action`: start | stop | reboot | force-stop.
pub fn power(id: &str, action: &str) -> Request {
    Request::post(
        format!("/services/{}/power", seg(id)),
        json!({ "action": action }),
    )
}
pub fn rename_server(id: &str, hostname: &str) -> Request {
    Request::patch(
        format!("/services/{}", seg(id)),
        json!({ "hostname": hostname }),
    )
}
pub fn resize_server(
    id: &str,
    cpu: Option<u32>,
    ram_mb: Option<u32>,
    disk_gb: Option<u32>,
    auto_reboot: Option<bool>,
) -> Request {
    Request::post(
        format!("/services/{}/resize", seg(id)),
        compact(
            json!({ "cpu": cpu, "ramMb": ram_mb, "diskGb": disk_gb, "autoReboot": auto_reboot }),
        ),
    )
}
pub fn rebuild_server(id: &str, os_template: &str, user_data: Option<&str>) -> Request {
    Request::post(
        format!("/services/{}/rebuild", seg(id)),
        compact(json!({ "osTemplate": os_template, "userData": user_data })),
    )
}
pub fn reset_password(id: &str) -> Request {
    Request::post(format!("/services/{}/reset-password", seg(id)), json!({}))
}

// ─── Snapshots ──────────────────────────────────────────────────────────

pub fn account_snapshots() -> Request {
    Request::get("/account/snapshots")
}
pub fn snapshots(server: &str) -> Request {
    Request::get(format!("/services/{}/snapshots", seg(server)))
}
pub fn create_snapshot(server: &str, name: &str) -> Request {
    Request::post(
        format!("/services/{}/snapshots", seg(server)),
        json!({ "name": name }),
    )
}
pub fn delete_snapshot(server: &str, snapshot: &str) -> Request {
    Request::delete(format!(
        "/services/{}/snapshots/{}",
        seg(server),
        seg(snapshot)
    ))
}
pub fn restore_snapshot(server: &str, snapshot: &str) -> Request {
    Request::post(
        format!(
            "/services/{}/snapshots/{}/restore",
            seg(server),
            seg(snapshot)
        ),
        json!({}),
    )
}
pub fn spawn_snapshot(
    server: &str,
    snapshot: &str,
    hostname: Option<&str>,
    plan_id: Option<&str>,
) -> Request {
    Request::post(
        format!(
            "/services/{}/snapshots/{}/spawn",
            seg(server),
            seg(snapshot)
        ),
        compact(json!({ "hostname": hostname, "planId": plan_id })),
    )
    .idempotent()
}

// ─── SSH keys ───────────────────────────────────────────────────────────

pub fn ssh_keys() -> Request {
    Request::get("/ssh-keys")
}
pub fn add_ssh_key(name: &str, public_key: &str) -> Request {
    Request::post(
        "/ssh-keys",
        json!({ "name": name, "publicKey": public_key }),
    )
}
pub fn delete_ssh_key(id: &str) -> Request {
    Request::delete(format!("/ssh-keys/{}", seg(id)))
}

// ─── Networking ─────────────────────────────────────────────────────────

pub fn reserved_ips() -> Request {
    Request::get("/account/reserved-ips")
}
pub fn reserve_ips(region: &str, family: Option<&str>, count: Option<u32>) -> Request {
    Request::post(
        "/account/reserved-ips",
        compact(json!({ "region": region, "family": family, "count": count })),
    )
    .idempotent()
}
pub fn attach_reserved_ip(id: &str, server: &str) -> Request {
    Request::post(
        format!("/account/reserved-ips/{}/attach", seg(id)),
        json!({ "serviceId": server }),
    )
}
pub fn detach_reserved_ip(id: &str) -> Request {
    Request::post(
        format!("/account/reserved-ips/{}/detach", seg(id)),
        json!({}),
    )
}
pub fn reserved_ip_auto_renew(id: &str, on: bool) -> Request {
    Request::patch(
        format!("/account/reserved-ips/{}/auto-renew", seg(id)),
        json!({ "autoRenew": on }),
    )
}
pub fn release_reserved_ip(id: &str) -> Request {
    Request::delete(format!("/account/reserved-ips/{}", seg(id)))
}

pub fn server_ips(server: &str) -> Request {
    Request::get(format!("/services/{}/ips", seg(server)))
}
pub fn attach_ip(server: &str, family: Option<&str>, reserved_ip: Option<&str>) -> Request {
    let body = match reserved_ip {
        Some(ip) => json!({ "ip": ip, "source": "reserved" }),
        None => compact(json!({ "family": family, "source": "pool" })),
    };
    Request::post(format!("/services/{}/ips", seg(server)), body)
}
pub fn detach_ip(server: &str, ip: &str) -> Request {
    Request::delete(format!("/services/{}/ips/{}", seg(server), seg(ip)))
}

pub fn firewall_rules(server: &str) -> Request {
    Request::get(format!("/services/{}/firewall", seg(server)))
}
pub fn add_firewall_rule(
    server: &str,
    direction: &str,
    protocol: &str,
    port: &str,
    source: &str,
    action: &str,
) -> Request {
    Request::post(
        format!("/services/{}/firewall", seg(server)),
        json!({ "direction": direction, "protocol": protocol, "port": port, "source": source, "action": action }),
    )
}
pub fn delete_firewall_rule(server: &str, rule: &str) -> Request {
    Request::delete(format!("/services/{}/firewall/{}", seg(server), seg(rule)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_path_escaped() {
        assert_eq!(server("a/b").path, "/services/a%2Fb");
        assert_eq!(
            detach_ip("s1", "2001:db8::1").path,
            "/services/s1/ips/2001%3Adb8%3A%3A1"
        );
    }

    #[test]
    fn unset_options_are_omitted() {
        let r = resize_server("s", Some(4), None, None, None);
        assert_eq!(r.body.unwrap(), json!({ "cpu": 4 }));
        let c = CreateServer {
            plan_id: "p".into(),
            region: "fra".into(),
            hostname: "h".into(),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            json!({ "planId": "p", "region": "fra", "hostname": "h" })
        );
        assert!(create_server(&c).idempotent);
    }
}
