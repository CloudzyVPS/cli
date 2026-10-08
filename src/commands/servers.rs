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
    col("Hourly price", "/priceHourly"),
    col("Monthly price", "/priceMonthly"),
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
        /// Preview resource and recurring price changes without applying them
        #[arg(long)]
        dry_run: bool,
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
    /// Request automatic backups (currently unavailable through the developer API)
    #[arg(long)]
    pub backups: bool,
    /// Preview resources and IPv4-inclusive pricing without creating a server
    #[arg(long, conflicts_with = "wait")]
    pub dry_run: bool,
    /// Wait until the server is active
    #[arg(long)]
    pub wait: bool,
}

pub async fn run(ctx: &Ctx, cmd: &ServersCommand) -> Result<()> {
    let api = ctx.api()?;
    match cmd {
        ServersCommand::List => {
            let resp = api.send(ops::servers()).await?;
            let mut rows = items(&resp);
            if !ctx.json() {
                resolve_plan_labels(&api, &mut rows).await;
            }
            print_list(
                ctx.format,
                &resp,
                &rows,
                LIST_COLS,
                "No servers yet. Create one with `zy servers create`.",
            );
        }
        ServersCommand::Get { id } => {
            let resp = api.send(ops::server(id)).await?;
            if ctx.json() {
                print_json(&resp);
            } else {
                let mut rows = vec![resp];
                resolve_plan_labels(&api, &mut rows).await;
                print_object(ctx.format, &rows[0], DETAIL_COLS);
            }
        }
        ServersCommand::Create(args) => {
            let body = build_create(&api, args).await?;
            let quote = super::catalog::quote_plan(
                &api,
                &body.plan_id,
                &body.region,
                body.billing_cycle.as_deref().unwrap_or("monthly"),
                body.ip_version.as_deref() != Some("ipv6"),
            )
            .await?;
            if args.dry_run {
                super::catalog::print_quote(ctx, &quote);
                return Ok(());
            }
            super::catalog::require_capacity(&quote)?;
            eprintln!("Configuration price before creation: {}. Catalog stock is advisory; the backend checks live capacity.", super::catalog::quote_summary(&quote["quote"]));
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
            dry_run,
        } => {
            if cpu.is_none() && ram_mb.is_none() && disk_gb.is_none() {
                return Err(CliError::Usage(
                    "give at least one of --cpu, --ram-mb, --disk-gb".into(),
                ));
            }
            let preview = resize_preview(&api, id, *cpu, *ram_mb, *disk_gb).await?;
            if *dry_run {
                print_object(ctx.format, &preview, &[]);
                return Ok(());
            }
            eprintln!("Resize: CPU {} → {}, RAM {} → {} MB, disk {} → {} GB. Current monthly price: {}. Requested price: {}. Hourly services re-rate immediately; prepaid services re-rate at renewal. No immediate charge is quoted.",
                preview["current"]["cpu"], preview["requested"]["cpu"], preview["current"]["ramMb"], preview["requested"]["ramMb"],
                preview["current"]["diskGb"], preview["requested"]["diskGb"], preview["current"]["priceMonthly"], super::catalog::quote_summary(&preview["configurationQuote"]));
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
            // A successful synchronous resize updates configuration before returning.
            // Read back the applied prices rather than assuming the preview was applied.
            match api.send(ops::server(id)).await {
                Ok(server) => {
                    if !ctx.json() { print_object(ctx.format, &server, DETAIL_COLS); }
                    else { eprintln!("Applied resources and prices: cpu={}, ramMb={}, diskGb={}, priceHourly={}, priceMonthly={}", server["cpu"], server["ramMb"], server["diskGb"], server["priceHourly"], server["priceMonthly"]); }
                }
                Err(error) => eprintln!("Resize was accepted but readback failed: {error}. Inspect `zy servers get {id}` before retrying."),
            }
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
            if ctx.json() {
                print_json(&resp);
            } else {
                print_object(
                    ctx.format,
                    &usage_display(&resp),
                    &[
                        col("CPU", "/cpu"),
                        col("RAM", "/ram"),
                        col("Disk", "/disk"),
                        col("Bandwidth", "/bandwidth"),
                    ],
                );
            }
        }
        ServersCommand::Activity { id } => {
            let resp = api.send(ops::server_activity(id)).await?;
            let rows: Vec<Value> = items(&resp).iter().map(activity_display).collect();
            print_list(
                ctx.format,
                &resp,
                &rows,
                &[
                    col("WHEN", "/createdAt"),
                    col("TRANSITION", "/_transition"),
                    col("REASON", "/reason"),
                    col("ACTOR", "/_actor"),
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
        let remaining = timeout.saturating_sub(started.elapsed());
        let server = tokio::time::timeout(remaining, api.send(ops::server(id))).await
            .map_err(|_| CliError::Other(format!("timed out after {}s waiting for server {id}. Provisioning may continue; inspect `zy servers get {id}` or resume `zy servers wait {id}` before creating again", timeout.as_secs())))??;
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
                "timed out after {}s; server {id} is still {state}. Provisioning may continue; inspect `zy servers get {id}` or resume `zy servers wait {id}` rather than creating again",
                timeout.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_secs(5).min(timeout.saturating_sub(started.elapsed())))
            .await;
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
    if a.backups {
        return Err(CliError::Usage("automatic backups cannot currently be enabled and verified through the developer API; configure them in the Cloudzy dashboard before relying on them".into()));
    }
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

fn usage_display(response: &Value) -> Value {
    let mut display = json!({});
    for name in ["cpu", "ram", "disk", "bandwidth"] {
        let metric = &response[name];
        let text = if metric["available"] == false || !metric["current"].is_number() {
            "unavailable".into()
        } else {
            let current = &metric["current"];
            let unit = metric["unit"].as_str().unwrap_or("");
            if unit == "%" {
                format!("{current}%")
            } else if metric["limit"].is_number() {
                format!("{current} / {} {unit}", metric["limit"])
            } else {
                format!("{current} {unit}")
            }
        };
        display[name] = json!(text);
    }
    display
}

fn activity_display(event: &Value) -> Value {
    let mut display = event.clone();
    let from = event["fromState"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("(new)");
    let to = event["toState"].as_str().unwrap_or("-");
    display["_transition"] = json!(format!("{from} → {to}"));
    display["_actor"] = json!(event["actor"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| event["actorType"].as_str())
        .unwrap_or("-"));
    display
}

async fn resolve_plan_labels(api: &ApiClient, rows: &mut [Value]) {
    if !rows.iter().any(|r| {
        r["plan"]["id"].is_string()
            && (r["plan"]["name"] == r["hostname"] || !r["plan"]["name"].is_string())
    }) {
        return;
    }
    let catalog = match api.send(ops::pricing_catalog()).await {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("Could not resolve catalog plan labels: {error}");
            for row in rows {
                if row["plan"]["name"] == row["hostname"] {
                    row["plan"]["name"] = row["plan"]["id"].clone();
                }
            }
            return;
        }
    };
    let plans = catalog.get("plans").map(items).unwrap_or_default();
    for row in rows {
        if let Some(plan) = plans
            .iter()
            .find(|p| p["id"].is_string() && p["id"] == row["plan"]["id"])
        {
            row["plan"]["name"] = plan["name"].clone();
        } else if row["plan"]["name"] == row["hostname"] {
            row["plan"]["name"] = row["plan"]["id"].clone();
        }
    }
}

async fn resize_preview(
    api: &ApiClient,
    id: &str,
    cpu: Option<u32>,
    ram_mb: Option<u32>,
    disk_gb: Option<u32>,
) -> Result<Value> {
    let source = api.send(ops::server(id)).await?;
    let plan_id = source["planId"]
        .as_str()
        .or_else(|| source["plan"]["id"].as_str())
        .ok_or_else(|| {
            CliError::Usage(
                "server has no catalog plan id; an accurate resize quote requires the dashboard"
                    .into(),
            )
        })?;
    let catalog = api.send(ops::pricing_catalog()).await?;
    let plan = items(&catalog["plans"])
        .into_iter()
        .find(|p| p["id"].as_str() == Some(plan_id))
        .ok_or_else(|| {
            CliError::Usage(
                "server plan is absent from the catalog; use the dashboard to quote this resize"
                    .into(),
            )
        })?;
    let mut requested = source.clone();
    for (key, value) in [("cpu", cpu), ("ramMb", ram_mb), ("diskGb", disk_gb)] {
        if let Some(value) = value {
            if value == 0 {
                return Err(CliError::Usage(format!("{key} must be positive")));
            }
            requested[key] = json!(value);
        }
    }
    if requested["diskGb"].as_u64() < source["diskGb"].as_u64() {
        return Err(CliError::Usage("disk cannot shrink".into()));
    }
    let before = api
        .pricing_quote(existing_quote_body(&source, &plan)?)
        .await?;
    // The generic pricing endpoint has no service context. Refuse when its
    // reconstructed price disagrees with this service's persisted rate.
    let observed = source["priceMonthly"].as_f64().ok_or_else(|| {
        CliError::Usage(
            "server has no current monthly rate; use the dashboard to quote the resize".into(),
        )
    })?;
    let quoted = before["subtotalMonthlyCents"]
        .as_f64()
        .ok_or_else(|| CliError::Other("pricing response omitted the monthly total".into()))?;
    if (observed * 100.0 - quoted).abs() > 0.01 {
        return Err(CliError::Usage("pricing quote does not match the current service rate (possibly custom features or pinned pricing); use the dashboard for an authoritative resize quote".into()));
    }
    let after = api
        .pricing_quote(existing_quote_body(&requested, &plan)?)
        .await?;
    super::catalog::validate_quote(&after)?;
    Ok(
        json!({"server": id, "current": {"cpu": source["cpu"], "ramMb": source["ramMb"], "diskGb": source["diskGb"],
        "priceMonthly": source["priceMonthly"], "priceHourly": source["priceHourly"]},
        "requested": {"cpu": requested["cpu"], "ramMb": requested["ramMb"], "diskGb": requested["diskGb"]},
        "configurationQuote": after, "immediateCharge": null, "billingEffect": "hourly: immediate re-rate; prepaid: next renewal; no immediate charge quoted"}),
    )
}

pub(crate) fn existing_quote_body(server: &Value, plan: &Value) -> Result<Value> {
    let n = |value: &Value, key: &str| {
        value[key].as_u64().ok_or_else(|| {
            CliError::Usage(format!(
                "missing {key}; cannot reconstruct an accurate quote"
            ))
        })
    };
    let (cpu, ram, disk) = (n(server, "cpu")?, n(server, "ramMb")?, n(server, "diskGb")?);
    let (base_cpu, base_ram, base_disk) = (
        n(plan, "cpuCores")?,
        n(plan, "memoryMb")?,
        n(plan, "diskGb")?,
    );
    if server["nested"] == true || server["autoBackups"] == true {
        return Err(CliError::Usage(
            "the generic quote cannot verify this server's feature prices; use the dashboard"
                .into(),
        ));
    }
    let region = server["region"]
        .as_str()
        .ok_or_else(|| CliError::Usage("server region missing".into()))?;
    let cycle = server["billingCycle"]
        .as_str()
        .ok_or_else(|| CliError::Usage("server billing cycle missing".into()))?;
    let ipv4 = server["ipAddress"]
        .as_str()
        .is_some_and(|ip| !ip.is_empty());
    Ok(
        json!({"planId": plan["id"], "region": region, "billingCycle": cycle, "quantity": 1,
        "includeIpv4": ipv4, "extras": {"cpuCores": cpu.saturating_sub(base_cpu),
        "ramGb": (ram.saturating_add(512) / 1024).saturating_sub(base_ram.saturating_add(512) / 1024),
        "diskGb": disk.saturating_sub(base_disk),
        "bandwidthTb": server["bandwidthTb"].as_u64().unwrap_or(0).saturating_sub(plan["bandwidthTb"].as_u64().unwrap_or(0))}}),
    )
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
