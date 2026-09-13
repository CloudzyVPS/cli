//! The MCP tools: one per Cloudzy operation, defined once, dispatched here.

use serde_json::{json, Map, Value};

use crate::api::{items, ops, ApiClient, Request};
use crate::commands::servers::{resolve_plan, resolve_ssh_keys};
use crate::error::CliError;

/// Behaviour hints clients use to decide when to ask the person first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Reads only.
    Read,
    /// Changes something, but can be undone or only adds.
    Write,
    /// Destroys data, replaces credentials, or costs money irreversibly.
    Destructive,
}

pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    /// Properties of the input object; `required` lists mandatory ones.
    pub properties: Value,
    pub required: &'static [&'static str],
}

impl Tool {
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": self.properties,
                "required": self.required,
                "additionalProperties": false,
            },
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.kind == Kind::Read,
                "destructiveHint": self.kind == Kind::Destructive,
                "idempotentHint": self.kind == Kind::Read,
                "openWorldHint": true,
            }
        })
    }
}

fn s(desc: &str) -> Value {
    json!({ "type": "string", "description": desc })
}
fn int(desc: &str) -> Value {
    json!({ "type": "integer", "minimum": 1, "description": desc })
}
fn boolean(desc: &str) -> Value {
    json!({ "type": "boolean", "description": desc })
}
fn one_of(desc: &str, values: &[&str]) -> Value {
    json!({ "type": "string", "enum": values, "description": desc })
}
fn strings(desc: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": desc })
}

const SERVER_ID: &str = "Server id (from list_servers)";

pub fn all() -> Vec<Tool> {
    use Kind::*;
    let t = |name, title, description, kind, properties: Value, required| Tool {
        name,
        title,
        description,
        kind,
        properties,
        required,
    };
    vec![
        t("whoami", "Who am I", "The Cloudzy account the credential belongs to.", Read, json!({}), &[]),
        // Servers
        t("list_servers", "List servers", "All servers on the account with state, power, region, IPs and size.", Read, json!({}), &[]),
        t("get_server", "Get server", "Full details of one server.", Read, json!({ "id": s(SERVER_ID) }), &["id"]),
        t("create_server", "Create server",
          "Create a server. Charged to the account balance immediately. Poll get_server until state is \"active\". Use list_plans, list_regions, list_os_templates and list_apps to choose values.",
          Write,
          json!({
            "hostname": s("Hostname"),
            "plan": s("Plan id or slug"),
            "region": s("Region id"),
            "osTemplateId": s("OS template id, e.g. ubuntu-24.04"),
            "billingCycle": s("hourly, monthly, quarterly, …"),
            "sshKeys": strings("Saved SSH key names or ids to install for root"),
            "sshPublicKeys": strings("OpenSSH public key lines to install for root"),
            "app": s("One-click app name"),
            "appParams": { "type": "object", "additionalProperties": { "type": "string" }, "description": "One-click app parameters" },
            "userData": s("cloud-init user data"),
            "ipVersion": one_of("IP stack", &["ipv4", "ipv6", "dual"]),
          }),
          &["hostname", "plan", "region"]),
        t("delete_server", "Delete server", "Destroy a server and all its data. Irreversible; confirm with the person first.", Destructive, json!({ "id": s(SERVER_ID) }), &["id"]),
        t("power_server", "Power action", "Start, stop, reboot or force-stop a server.", Write,
          json!({ "id": s(SERVER_ID), "action": one_of("Power action", &["start", "stop", "reboot", "force-stop"]) }), &["id", "action"]),
        t("rename_server", "Rename server", "Change a server's hostname.", Write, json!({ "id": s(SERVER_ID), "hostname": s("New hostname") }), &["id", "hostname"]),
        t("resize_server", "Resize server", "Grow vCPU, RAM (MB) or total disk (GB). Shrinking is not supported. Reboots unless autoReboot is false.", Write,
          json!({ "id": s(SERVER_ID), "cpu": int("vCPU count"), "ramMb": int("RAM in MB"), "diskGb": int("Total disk in GB"), "autoReboot": boolean("Reboot to apply (default true)") }), &["id"]),
        t("rebuild_server", "Rebuild server", "Reinstall a server from an OS template, erasing its disk. Returns the new root password.", Destructive,
          json!({ "id": s(SERVER_ID), "osTemplateId": s("OS template id"), "userData": s("cloud-init user data; omit to keep the existing one") }), &["id", "osTemplateId"]),
        t("reset_server_password", "Reset root password", "Replace a server's root password and return the new one.", Destructive, json!({ "id": s(SERVER_ID) }), &["id"]),
        t("server_status", "Server power state", "Live power state of a server.", Read, json!({ "id": s(SERVER_ID) }), &["id"]),
        t("server_usage", "Server usage", "CPU, RAM, disk and bandwidth usage.", Read, json!({ "id": s(SERVER_ID) }), &["id"]),
        t("server_activity", "Server activity", "Activity log of a server.", Read, json!({ "id": s(SERVER_ID) }), &["id"]),
        // Snapshots
        t("list_snapshots", "List snapshots", "Snapshots of one server, or of every server when server is omitted.", Read, json!({ "server": s(SERVER_ID) }), &[]),
        t("create_snapshot", "Create snapshot", "Snapshot a server (at most five per server; billed while kept).", Write, json!({ "server": s(SERVER_ID), "name": s("Snapshot name") }), &["server", "name"]),
        t("delete_snapshot", "Delete snapshot", "Delete a snapshot.", Destructive, json!({ "server": s(SERVER_ID), "snapshot": s("Snapshot id") }), &["server", "snapshot"]),
        t("restore_snapshot", "Restore snapshot", "Roll a server back to a snapshot, discarding everything written since.", Destructive, json!({ "server": s(SERVER_ID), "snapshot": s("Snapshot id") }), &["server", "snapshot"]),
        t("spawn_server_from_snapshot", "New server from snapshot", "Create a new server from a snapshot. Charged like create_server.", Write,
          json!({ "server": s(SERVER_ID), "snapshot": s("Snapshot id"), "hostname": s("Hostname for the new server"), "plan": s("Plan id; defaults to the source server's plan") }), &["server", "snapshot"]),
        // SSH keys
        t("list_ssh_keys", "List SSH keys", "Saved SSH public keys.", Read, json!({}), &[]),
        t("add_ssh_key", "Add SSH key", "Save an OpenSSH public key.", Write, json!({ "name": s("Key name"), "publicKey": s("OpenSSH public key line") }), &["name", "publicKey"]),
        t("delete_ssh_key", "Delete SSH key", "Delete a saved SSH key.", Destructive, json!({ "id": s("SSH key id") }), &["id"]),
        // Networking
        t("list_reserved_ips", "List reserved IPs", "Reserved (floating) IPs on the account.", Read, json!({}), &[]),
        t("reserve_ips", "Reserve IPs", "Reserve new public IPs in a region, billed monthly.", Write,
          json!({ "region": s("Region id"), "family": one_of("Address family", &["ipv4", "ipv6"]), "count": int("How many (1-30)") }), &["region"]),
        t("attach_reserved_ip", "Attach reserved IP", "Attach a reserved IP to a server.", Write, json!({ "id": s("Reserved IP id"), "server": s(SERVER_ID) }), &["id", "server"]),
        t("detach_reserved_ip", "Detach reserved IP", "Detach a reserved IP from its server; it stays reserved and billed.", Write, json!({ "id": s("Reserved IP id") }), &["id"]),
        t("set_reserved_ip_auto_renew", "Reserved IP auto-renew", "Turn monthly auto-renew on or off.", Write, json!({ "id": s("Reserved IP id"), "enabled": boolean("Auto-renew") }), &["id", "enabled"]),
        t("release_reserved_ip", "Release reserved IP", "Give a reserved IP back to the pool. Non-refundable; the address may not be recoverable.", Destructive, json!({ "id": s("Reserved IP id") }), &["id"]),
        t("list_server_ips", "List server IPs", "Public IPs attached to a server.", Read, json!({ "server": s(SERVER_ID) }), &["server"]),
        t("attach_server_ip", "Attach IP to server", "Attach an extra IP from the pool, or one of the account's reserved IPs by address.", Write,
          json!({ "server": s(SERVER_ID), "family": one_of("Pool address family", &["ipv4", "ipv6"]), "reservedIp": s("Reserved IP address to attach instead of a pool address") }), &["server"]),
        t("detach_server_ip", "Detach IP from server", "Detach an extra IP from a server.", Destructive, json!({ "server": s(SERVER_ID), "ip": s("IP address") }), &["server", "ip"]),
        t("list_firewall_rules", "List firewall rules", "A server's firewall rules.", Read, json!({ "server": s(SERVER_ID) }), &["server"]),
        t("add_firewall_rule", "Add firewall rule", "Add a firewall rule to a server.", Write,
          json!({ "server": s(SERVER_ID), "direction": one_of("Direction", &["in", "out"]), "protocol": s("tcp, udp, icmp or any"),
                  "port": s("Port or range, e.g. 22 or 8000-8100"), "source": s("Source CIDR"), "action": one_of("Action", &["allow", "deny"]) }),
          &["server", "direction", "protocol", "action"]),
        t("delete_firewall_rule", "Delete firewall rule", "Delete a firewall rule.", Destructive, json!({ "server": s(SERVER_ID), "rule": s("Rule id") }), &["server", "rule"]),
        // Catalog
        t("list_regions", "List regions", "Regions servers can be created in, with stock status.", Read, json!({}), &[]),
        t("list_plans", "List plans", "Plans with specs and prices; prices are in cents. Pass region to get that region's prices.", Read,
          json!({ "region": s("Region id"), "billingCycle": s("Billing cycle to price (default monthly)") }), &[]),
        t("list_os_templates", "List OS templates", "Installable operating systems.", Read, json!({}), &[]),
        t("list_apps", "List one-click apps", "One-click apps that can be installed at create time.", Read, json!({}), &[]),
        t("get_app", "Get one-click app", "A one-click app and the parameters it takes.", Read, json!({ "name": s("App name") }), &["name"]),
        // Billing
        t("get_balance", "Balance", "Wallet balance in cents.", Read, json!({}), &[]),
        t("list_ledger", "Ledger", "Wallet ledger entries, newest first.", Read, json!({ "page": int("Page"), "perPage": int("Entries per page") }), &[]),
        t("list_invoices", "Invoices", "Invoices, newest first.", Read, json!({ "page": int("Page"), "perPage": int("Invoices per page") }), &[]),
    ]
}

/// A failed tool call: the message the model sees, and for API failures the
/// structured error.
#[derive(Debug)]
pub struct ToolError {
    pub message: String,
    pub detail: Option<Value>,
}

impl From<CliError> for ToolError {
    fn from(e: CliError) -> Self {
        let detail = match &e {
            CliError::Api(api) => Some(api.to_json()),
            _ => None,
        };
        ToolError {
            message: e.to_string(),
            detail,
        }
    }
}

impl From<crate::api::ApiError> for ToolError {
    fn from(e: crate::api::ApiError) -> Self {
        ToolError::from(CliError::Api(e))
    }
}

fn arg_err(msg: String) -> ToolError {
    ToolError {
        message: msg,
        detail: None,
    }
}

struct Args<'a>(&'a Map<String, Value>);

impl<'a> Args<'a> {
    fn str(&self, k: &str) -> Result<&'a str, ToolError> {
        self.opt_str(k)
            .ok_or_else(|| arg_err(format!("missing required argument: {k}")))
    }
    fn opt_str(&self, k: &str) -> Option<&'a str> {
        self.0
            .get(k)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    }
    fn opt_u32(&self, k: &str) -> Result<Option<u32>, ToolError> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(Some)
                .ok_or_else(|| arg_err(format!("{k} must be a positive integer"))),
        }
    }
    fn opt_bool(&self, k: &str) -> Result<Option<bool>, ToolError> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            _ => Err(arg_err(format!("{k} must be true or false"))),
        }
    }
    fn strings(&self, k: &str) -> Result<Vec<String>, ToolError> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| arg_err(format!("{k} must be a list of strings")))
                })
                .collect(),
            _ => Err(arg_err(format!("{k} must be a list of strings"))),
        }
    }
    fn one_of(&self, k: &str, allowed: &[&str]) -> Result<&'a str, ToolError> {
        let v = self.str(k)?;
        if allowed.contains(&v) {
            Ok(v)
        } else {
            Err(arg_err(format!(
                "{k} must be one of: {}",
                allowed.join(", ")
            )))
        }
    }
}

/// Run a tool. `Ok` carries the API's JSON.
pub async fn call(api: &ApiClient, name: &str, arguments: &Value) -> Result<Value, ToolError> {
    let empty = Map::new();
    let a = Args(arguments.as_object().unwrap_or(&empty));
    let req: Request = match name {
        "whoami" => ops::whoami(),
        "list_servers" => ops::servers(),
        "get_server" => ops::server(a.str("id")?),
        "create_server" => {
            let mut keys = resolve_ssh_keys(api, &a.strings("sshKeys")?).await?;
            keys.extend(a.strings("sshPublicKeys")?);
            let mut config = Map::new();
            if let Some(app) = a.opt_str("app") {
                config.insert("ocaName".into(), json!(app));
                if let Some(p) = a.0.get("appParams").filter(|p| p.is_object()) {
                    config.insert("ocaParams".into(), p.clone());
                }
            }
            if let Some(ud) = a.opt_str("userData") {
                config.insert("userData".into(), json!(ud));
            }
            ops::create_server(&ops::CreateServer {
                plan_id: resolve_plan(api, a.str("plan")?).await?,
                region: a.str("region")?.into(),
                hostname: a.str("hostname")?.into(),
                billing_cycle: a.opt_str("billingCycle").map(Into::into),
                os_template_id: a.opt_str("osTemplateId").map(Into::into),
                ip_version: a.opt_str("ipVersion").map(Into::into),
                ssh_authorized_keys: keys,
                config,
                ..Default::default()
            })
        }
        "delete_server" => ops::delete_server(a.str("id")?),
        "power_server" => ops::power(
            a.str("id")?,
            a.one_of("action", &["start", "stop", "reboot", "force-stop"])?,
        ),
        "rename_server" => ops::rename_server(a.str("id")?, a.str("hostname")?),
        "resize_server" => {
            let (cpu, ram, disk) = (a.opt_u32("cpu")?, a.opt_u32("ramMb")?, a.opt_u32("diskGb")?);
            if cpu.is_none() && ram.is_none() && disk.is_none() {
                return Err(arg_err("give at least one of cpu, ramMb, diskGb".into()));
            }
            ops::resize_server(a.str("id")?, cpu, ram, disk, a.opt_bool("autoReboot")?)
        }
        "rebuild_server" => {
            ops::rebuild_server(a.str("id")?, a.str("osTemplateId")?, a.opt_str("userData"))
        }
        "reset_server_password" => ops::reset_password(a.str("id")?),
        "server_status" => ops::server_status(a.str("id")?),
        "server_usage" => ops::server_usage(a.str("id")?),
        "server_activity" => ops::server_activity(a.str("id")?),
        "list_snapshots" => match a.opt_str("server") {
            Some(srv) => ops::snapshots(srv),
            None => ops::account_snapshots(),
        },
        "create_snapshot" => ops::create_snapshot(a.str("server")?, a.str("name")?),
        "delete_snapshot" => ops::delete_snapshot(a.str("server")?, a.str("snapshot")?),
        "restore_snapshot" => ops::restore_snapshot(a.str("server")?, a.str("snapshot")?),
        "spawn_server_from_snapshot" => ops::spawn_snapshot(
            a.str("server")?,
            a.str("snapshot")?,
            a.opt_str("hostname"),
            a.opt_str("plan"),
        ),
        "list_ssh_keys" => ops::ssh_keys(),
        "add_ssh_key" => ops::add_ssh_key(a.str("name")?, a.str("publicKey")?),
        "delete_ssh_key" => ops::delete_ssh_key(a.str("id")?),
        "list_reserved_ips" => ops::reserved_ips(),
        "reserve_ips" => {
            ops::reserve_ips(a.str("region")?, a.opt_str("family"), a.opt_u32("count")?)
        }
        "attach_reserved_ip" => ops::attach_reserved_ip(a.str("id")?, a.str("server")?),
        "detach_reserved_ip" => ops::detach_reserved_ip(a.str("id")?),
        "set_reserved_ip_auto_renew" => ops::reserved_ip_auto_renew(
            a.str("id")?,
            a.opt_bool("enabled")?
                .ok_or_else(|| arg_err("missing required argument: enabled".into()))?,
        ),
        "release_reserved_ip" => ops::release_reserved_ip(a.str("id")?),
        "list_server_ips" => ops::server_ips(a.str("server")?),
        "attach_server_ip" => ops::attach_ip(
            a.str("server")?,
            a.opt_str("family"),
            a.opt_str("reservedIp"),
        ),
        "detach_server_ip" => ops::detach_ip(a.str("server")?, a.str("ip")?),
        "list_firewall_rules" => ops::firewall_rules(a.str("server")?),
        "add_firewall_rule" => ops::add_firewall_rule(
            a.str("server")?,
            a.one_of("direction", &["in", "out"])?,
            a.str("protocol")?,
            a.opt_str("port").unwrap_or(""),
            a.opt_str("source").unwrap_or("0.0.0.0/0"),
            a.one_of("action", &["allow", "deny"])?,
        ),
        "delete_firewall_rule" => ops::delete_firewall_rule(a.str("server")?, a.str("rule")?),
        "list_regions" => ops::regions(),
        "list_plans" => {
            return list_plans(
                api,
                a.opt_str("region"),
                a.opt_str("billingCycle").unwrap_or("monthly"),
            )
            .await
        }
        "list_os_templates" => ops::os_templates(),
        "list_apps" => ops::apps(),
        "get_app" => ops::app(a.str("name")?),
        "get_balance" => ops::balance(),
        "list_ledger" => ops::ledger(
            a.opt_u32("page")?.unwrap_or(1),
            a.opt_u32("perPage")?.unwrap_or(50),
        ),
        "list_invoices" => ops::invoices(
            a.opt_u32("page")?.unwrap_or(1),
            a.opt_u32("perPage")?.unwrap_or(20),
        ),
        other => return Err(arg_err(format!("unknown tool: {other}"))),
    };
    Ok(api.send(req).await?)
}

/// Plans joined with the one price that applies, instead of the whole
/// catalog: every plan × region × cycle would swamp a model's context.
async fn list_plans(
    api: &ApiClient,
    region: Option<&str>,
    cycle: &str,
) -> Result<Value, ToolError> {
    let catalog = api.send(ops::pricing_catalog()).await?;
    let prices = catalog.get("prices").map(items).unwrap_or_default();
    let plans: Vec<Value> = catalog
        .get("plans")
        .map(items)
        .unwrap_or_default()
        .into_iter()
        .map(|mut plan| {
            let id = plan
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let pick = |rid: &str| {
                prices.iter().find(|p| {
                    p.get("planId").and_then(Value::as_str) == Some(id.as_str())
                        && p.get("billingCycle").and_then(Value::as_str) == Some(cycle)
                        && p.get("regionId").and_then(Value::as_str).unwrap_or("") == rid
                })
            };
            plan["price"] = region
                .and_then(pick)
                .or_else(|| pick(""))
                .cloned()
                .unwrap_or(Value::Null);
            plan
        })
        .collect();
    Ok(json!({ "billingCycle": cycle, "region": region, "plans": plans }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique_and_required_fields_exist() {
        let tools = all();
        let mut seen = HashSet::new();
        for t in &tools {
            assert!(seen.insert(t.name), "duplicate tool {}", t.name);
            assert!(
                t.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{}",
                t.name
            );
            for r in t.required {
                assert!(
                    t.properties.get(*r).is_some(),
                    "{} requires undeclared {r}",
                    t.name
                );
            }
        }
    }

    #[test]
    fn destructive_tools_are_marked() {
        let tools = all();
        let kind = |n: &str| tools.iter().find(|t| t.name == n).unwrap().kind;
        for n in [
            "delete_server",
            "rebuild_server",
            "reset_server_password",
            "restore_snapshot",
            "release_reserved_ip",
        ] {
            assert_eq!(kind(n), Kind::Destructive, "{n}");
        }
        assert_eq!(kind("list_servers"), Kind::Read);
        let j = tools
            .iter()
            .find(|t| t.name == "delete_server")
            .unwrap()
            .to_json();
        assert_eq!(j["annotations"]["destructiveHint"], true);
        assert_eq!(j["annotations"]["readOnlyHint"], false);
    }
}
