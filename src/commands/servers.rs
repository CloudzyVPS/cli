//! `zy servers …`

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{json, Map, Value};

use super::context::{confirm, Ctx};
use crate::api::{items, ops, ApiClient};
use crate::error::{CliError, Result};
use crate::output::{col, print_json, print_list, print_object, Col};

pub const LIST_COLS: &[Col] = &[
    col("ID", "/id"),
    col("HOSTNAME", "/hostname"),
    col("STATE", "/state"),
    col("POWER", "/powerState"),
    col("REGION", "/region"),
    col("IPV4", "/ipAddress"),
    col("VCPU", "/cpu"),
    col("RAM MB", "/ramMb"),
    col("DISK GB", "/diskGb"),
    col("PLAN", "/plan/name"),
];

pub const DETAIL_COLS: &[Col] = &[
    col("ID", "/id"),
    col("Hostname", "/hostname"),
    col("State", "/state"),
    col("Power", "/powerState"),
    col("Region", "/region"),
    col("IPv4", "/ipAddress"),
    col("IPv6", "/ipv6Address"),
    col("OS", "/osTemplate"),
    col("App", "/ocaName"),
    col("vCPU", "/cpu"),
    col("RAM (MB)", "/ramMb"),
    col("Disk (GB)", "/diskGb"),
    col("Bandwidth (TB)", "/bandwidthTb"),
    col("Plan", "/plan/name"),
    col("Billing cycle", "/billingCycle"),
    col("Next renewal", "/nextRenewalAt"),
    col("Auto-renew", "/autoRenew"),
    col("Reverse DNS", "/reverseDns"),
    col("Tags", "/tags"),
    col("Failure", "/failureReason"),
    col("Created", "/createdAt"),
];

#[derive(Subcommand, Debug)]
pub enum ServersCommand {
    /// List your servers
    #[command(visible_alias = "ls")]
    List,
    /// Show one server
    Get { id: String },
    /// Create a server (charged to your Cloudzy balance)
    Create(Box<CreateArgs>),
    /// Destroy a server permanently
    #[command(visible_alias = "rm")]
    Delete {
        id: String,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Start, stop, reboot or force-stop a server
    Power {
        id: String,
        #[arg(value_enum)]
        action: PowerAction,
    },
    /// Show live power state
    Status { id: String },
    /// Change a server's hostname
    Rename { id: String, hostname: String },
    /// Grow vCPU, RAM or disk (no shrinking)
    Resize {
        id: String,
        /// New vCPU count
        #[arg(long)]
        cpu: Option<u32>,
        /// New RAM in MB
        #[arg(long)]
        ram_mb: Option<u32>,
        /// New total disk in GB
        #[arg(long)]
        disk_gb: Option<u32>,
        /// Stage the change without rebooting (apply with a stop and start)
        #[arg(long)]
        no_reboot: bool,
    },
    /// Reinstall from an OS template, erasing the disk
    Rebuild {
        id: String,
        /// OS template id (see `zy os list`)
        #[arg(long)]
        os: String,
        /// cloud-init user data file; omit to keep the existing one
        #[arg(long, value_name = "FILE")]
        user_data: Option<std::path::PathBuf>,
        #[arg(long, short)]
        yes: bool,
    },
    /// Reset the root password and print the new one
    ResetPassword {
        id: String,
        #[arg(long, short)]
        yes: bool,
    },
    /// CPU, RAM, disk and bandwidth usage
    Usage { id: String },
    /// A server's activity log
    Activity { id: String },
    /// Wait until a server reaches a state
    Wait {
        id: String,
        /// Target state
        #[arg(long, default_value = "active")]
        state: String,
        /// Give up after this many seconds
        #[arg(long, default_value_t = 900)]
        timeout: u64,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PowerAction {
    Start,
    Stop,
    Reboot,
    ForceStop,
}

impl PowerAction {
    pub fn api_name(self) -> &'static str {
        match self {
            PowerAction::Start => "start",
            PowerAction::Stop => "stop",
            PowerAction::Reboot => "reboot",
            PowerAction::ForceStop => "force-stop",
        }
    }
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Hostname
    #[arg(long)]
    pub hostname: String,
    /// Plan id or slug (see `zy plans list`)
    #[arg(long)]
    pub plan: String,
    /// Region id (see `zy regions list`)
    #[arg(long)]
    pub region: String,
    /// OS template id (see `zy os list`)
    #[arg(long)]
    pub os: Option<String>,
    /// Billing cycle, e.g. hourly, monthly, quarterly
    #[arg(long)]
    pub cycle: Option<String>,
    /// One-click app name (see `zy apps list`)
    #[arg(long)]
    pub app: Option<String>,
    /// One-click app parameter, KEY=VALUE (repeatable)
    #[arg(long = "app-param", value_name = "KEY=VALUE")]
    pub app_params: Vec<String>,
    /// Saved SSH key name or id to install for root (repeatable)
    #[arg(long = "ssh-key", value_name = "NAME|ID")]
    pub ssh_keys: Vec<String>,
    /// OpenSSH public key file to install for root (repeatable)
    #[arg(long = "ssh-key-file", value_name = "FILE")]
    pub ssh_key_files: Vec<std::path::PathBuf>,
    /// cloud-init user data file
    #[arg(long, value_name = "FILE")]
    pub user_data: Option<std::path::PathBuf>,
    /// ipv4, ipv6 or dual
    #[arg(long)]
    pub ip_version: Option<String>,
    /// Reserved IP id to use (repeatable)
    #[arg(long = "reserved-ip", value_name = "ID")]
    pub reserved_ips: Vec<String>,
    /// Enable automatic backups
    #[arg(long)]
    pub backups: bool,
    /// Wait until the server is active
    #[arg(long)]
    pub wait: bool,
}

pub async fn run(ctx: &Ctx, cmd: &ServersCommand) -> Result<()> {
    let api = ctx.api()?;
    match cmd {
        ServersCommand::List => {
            let resp = api.send(ops::servers()).await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                LIST_COLS,
                "No servers yet. Create one with `zy servers create`.",
            );
        }
        ServersCommand::Get { id } => {
            let resp = api.send(ops::server(id)).await?;
            print_object(ctx.format, &resp, DETAIL_COLS);
        }
        ServersCommand::Create(args) => {
            let body = build_create(&api, args).await?;
            let resp = api.send(ops::create_server(&body)).await?;
            let id = resp
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if args.wait && !id.is_empty() {
                ctx.done(format!(
                    "Server {id} created; waiting for it to become active…"
                ));
                let fin =
                    wait_for_state(&api, &id, "active", Duration::from_secs(900), !ctx.json())
                        .await?;
                print_object(ctx.format, &fin, DETAIL_COLS);
            } else {
                print_object(ctx.format, &resp, DETAIL_COLS);
                if !ctx.json() {
                    ctx.done(format!("Server {id} is provisioning. `zy servers wait {id}` blocks until it is active."));
                }
            }
        }
        ServersCommand::Delete { id, yes } => {
            let server = api.send(ops::server(id)).await?;
            let hostname = server.get("hostname").and_then(Value::as_str).unwrap_or(id);
            confirm(
                *yes,
                &format!("This destroys server {hostname} ({id}) and its data"),
                hostname,
            )?;
            api.send(ops::delete_server(id)).await?;
            if ctx.json() {
                print_json(&json!({ "deleted": id }));
            } else {
                ctx.done(format!("Server {hostname} is being destroyed."));
            }
        }
        ServersCommand::Power { id, action } => {
            let resp = api.send(ops::power(id, action.api_name())).await?;
            if ctx.json() {
                print_json(&resp);
            } else {
                ctx.done(format!("{} requested for {id}.", action.api_name()));
            }
        }
        ServersCommand::Status { id } => {
            let resp = api.send(ops::server_status(id)).await?;
            print_object(
                ctx.format,
                &resp,
                &[
                    col("VM", "/vmName"),
                    col("State", "/vmState"),
                    col("IPv4", "/ipAddress"),
                ],
            );
        }
        ServersCommand::Rename { id, hostname } => {
            let resp = api.send(ops::rename_server(id, hostname)).await?;
            print_object(ctx.format, &resp, DETAIL_COLS);
        }
        ServersCommand::Resize {
            id,
            cpu,
            ram_mb,
            disk_gb,
            no_reboot,
        } => {
            if cpu.is_none() && ram_mb.is_none() && disk_gb.is_none() {
                return Err(CliError::Usage(
                    "give at least one of --cpu, --ram-mb, --disk-gb".into(),
                ));
            }
            let auto_reboot = no_reboot.then_some(false);
            let resp = api
                .send(ops::resize_server(id, *cpu, *ram_mb, *disk_gb, auto_reboot))
                .await?;
            print_object(
                ctx.format,
                &resp,
                &[
                    col("Applied live", "/appliedLive"),
                    col("Reboot required", "/rebootRequired"),
                    col("Rebooted", "/rebootDone"),
                    col("Notes", "/notes"),
                    col("Reboot error", "/rebootError"),
                ],
            );
        }
        ServersCommand::Rebuild {
            id,
            os,
            user_data,
            yes,
        } => {
            let user_data = user_data.as_ref().map(read_file).transpose()?;
            confirm(*yes, &format!("Rebuilding {id} erases its disk"), id)?;
            let resp = api
                .send(ops::rebuild_server(id, os, user_data.as_deref()))
                .await?;
            print_secret_result(ctx, &resp, "/rootPassword", "Rebuild started.");
        }
        ServersCommand::ResetPassword { id, yes } => {
            confirm(
                *yes,
                &format!("This replaces the root password of {id}"),
                id,
            )?;
            let resp = api.send(ops::reset_password(id)).await?;
            print_secret_result(ctx, &resp, "/password", "Root password reset.");
        }
        ServersCommand::Usage { id } => {
            let resp = api.send(ops::server_usage(id)).await?;
            print_object(
                ctx.format,
                &resp,
                &[
                    col("CPU %", "/cpu"),
                    col("RAM %", "/ram"),
                    col("Disk %", "/disk"),
                    col("Bandwidth used", "/bandwidth/current"),
                    col("Bandwidth limit", "/bandwidth/limit"),
                    col("Unit", "/bandwidth/unit"),
                ],
            );
        }
        ServersCommand::Activity { id } => {
            let resp = api.send(ops::server_activity(id)).await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                &[
                    col("WHEN", "/timestamp"),
                    col("TYPE", "/type"),
                    col("DESCRIPTION", "/description"),
                    col("ACTOR", "/actorName"),
                ],
                "No activity.",
            );
        }
        ServersCommand::Wait { id, state, timeout } => {
            let fin =
                wait_for_state(&api, id, state, Duration::from_secs(*timeout), !ctx.json()).await?;
            print_object(ctx.format, &fin, DETAIL_COLS);
        }
    }
    Ok(())
}

fn print_secret_result(ctx: &Ctx, resp: &Value, pointer: &str, note: &str) {
    if ctx.json() {
        print_json(resp);
        return;
    }
    ctx.done(note);
    if let Some(pw) = resp
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
    {
        println!("{pw}");
        eprintln!("  This password is shown once. Store it somewhere safe.");
    }
    if let Some(w) = resp.get("warning").and_then(Value::as_str) {
        eprintln!("  {w}");
    }
}

fn read_file(path: &std::path::PathBuf) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::Usage(format!("cannot read {}: {e}", path.display())))
}

/// States a server does not leave on its own.
const TERMINAL_STATES: &[&str] = &[
    "failed_provisioning",
    "terminated",
    "suspended_nonpayment",
    "suspended_abuse",
    "suspended_fraud_hold",
    "suspended_admin",
];

pub async fn wait_for_state(
    api: &ApiClient,
    id: &str,
    target: &str,
    timeout: Duration,
    progress: bool,
) -> Result<Value> {
    let started = Instant::now();
    let mut last = String::new();
    loop {
        let server = api.send(ops::server(id)).await?;
        let state = server
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        if state != last {
            if progress {
                eprintln!("  {id}: {state}");
            }
            last = state.clone();
        }
        if state == target {
            return Ok(server);
        }
        if TERMINAL_STATES.contains(&state.as_str()) {
            let reason = server
                .get("failureReason")
                .and_then(Value::as_str)
                .unwrap_or("");
            return Err(CliError::Other(format!(
                "server {id} is {state} and will not become {target}. {reason}"
            )));
        }
        if started.elapsed() > timeout {
            return Err(CliError::Other(format!(
                "timed out after {}s; server {id} is still {state}",
                timeout.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn looks_like_uuid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

pub async fn resolve_plan(api: &ApiClient, plan: &str) -> Result<String> {
    if looks_like_uuid(plan) {
        return Ok(plan.to_string());
    }
    let catalog = api.send(ops::pricing_catalog()).await?;
    let plans = catalog.get("plans").map(items).unwrap_or_default();
    plans
        .iter()
        .find(|p| {
            p.get("slug").and_then(Value::as_str) == Some(plan)
                || p.get("id").and_then(Value::as_str) == Some(plan)
                || p.get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|n| n.eq_ignore_ascii_case(plan))
        })
        .and_then(|p| p.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .ok_or_else(|| {
            CliError::Usage(format!(
                "no plan with id, slug or name {plan:?}; see `zy plans list`"
            ))
        })
}

/// Saved keys by name or id, as the public key lines the create call takes.
pub async fn resolve_ssh_keys(api: &ApiClient, wanted: &[String]) -> Result<Vec<String>> {
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let keys = items(&api.send(ops::ssh_keys()).await?);
    wanted
        .iter()
        .map(|w| {
            keys.iter()
                .find(|k| {
                    k.get("id").and_then(Value::as_str) == Some(w)
                        || k.get("name").and_then(Value::as_str) == Some(w)
                        || k.get("fingerprint").and_then(Value::as_str) == Some(w)
                })
                .and_then(|k| k.get("publicKey").and_then(Value::as_str))
                .map(str::to_string)
                .ok_or_else(|| {
                    CliError::Usage(format!(
                        "no saved SSH key named or with id {w:?}; see `zy ssh-keys list`"
                    ))
                })
        })
        .collect()
}

async fn build_create(api: &ApiClient, a: &CreateArgs) -> Result<ops::CreateServer> {
    let mut ssh = resolve_ssh_keys(api, &a.ssh_keys).await?;
    for f in &a.ssh_key_files {
        ssh.push(read_file(f)?.trim().to_string());
    }
    let mut config = Map::new();
    if let Some(app) = &a.app {
        config.insert("ocaName".into(), json!(app));
        let mut params = BTreeMap::new();
        for kv in &a.app_params {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| CliError::Usage(format!("--app-param {kv:?} is not KEY=VALUE")))?;
            params.insert(k.to_string(), v.to_string());
        }
        if !params.is_empty() {
            config.insert("ocaParams".into(), json!(params));
        }
    } else if !a.app_params.is_empty() {
        return Err(CliError::Usage("--app-param needs --app".into()));
    }
    if let Some(ud) = &a.user_data {
        config.insert("userData".into(), json!(read_file(ud)?));
    }
    Ok(ops::CreateServer {
        plan_id: resolve_plan(api, &a.plan).await?,
        region: a.region.clone(),
        hostname: a.hostname.clone(),
        billing_cycle: a.cycle.clone(),
        os_template_id: a.os.clone(),
        ip_version: a.ip_version.clone(),
        ssh_authorized_keys: ssh,
        reserved_ip_ids: a.reserved_ips.clone(),
        auto_backups: a.backups,
        config,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn uuid_shape() {
        assert!(super::looks_like_uuid(
            "3f2a1b4c-0000-4000-8000-00000000abcd"
        ));
        assert!(!super::looks_like_uuid("vps-2gb"));
    }
}
