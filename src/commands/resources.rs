//! `zy snapshots`, `zy ssh-keys`, `zy reserved-ips`, `zy ips`, `zy firewall`,
//! `zy whoami`, `zy billing`.

use clap::{Subcommand, ValueEnum};
use serde_json::{json, Value};

use super::context::{confirm, money, precise_money, Ctx};
use crate::api::{items, ops};
use crate::error::{CliError, Result};
use crate::output::{col, print_json, print_list, print_object, Col};

// ─── Snapshots ──────────────────────────────────────────────────────────

const SNAPSHOT_COLS: &[Col] = &[
    col("ID", "/id"),
    col("NAME", "/name"),
    col("SERVER", "/serviceId"),
    col("STATUS", "/status"),
    col("SIZE GB", "/sizeGb"),
    col("PROGRESS", "/progress"),
    col("CREATED", "/createdAt"),
];

#[derive(Subcommand, Debug)]
pub enum SnapshotsCommand {
    /// List snapshots of one server, or of every server
    #[command(visible_alias = "ls")]
    List {
        /// Only this server's snapshots
        #[arg(long)]
        server: Option<String>,
    },
    /// Snapshot a server (up to five per server; billed while kept)
    Create {
        server: String,
        #[arg(long)]
        name: String,
    },
    /// Delete a snapshot
    #[command(visible_alias = "rm")]
    Delete {
        server: String,
        snapshot: String,
        #[arg(long, short)]
        yes: bool,
    },
    /// Roll a server back to a snapshot, discarding newer changes
    Restore {
        server: String,
        snapshot: String,
        #[arg(long, short)]
        yes: bool,
    },
    /// Create a new server from a snapshot
    Spawn {
        server: String,
        snapshot: String,
        #[arg(long)]
        hostname: Option<String>,
        /// Plan id or slug; selected CPU/RAM are applied explicitly, disk cannot shrink
        #[arg(long)]
        plan: Option<String>,
        /// Billing cycle (defaults to the source server's cycle)
        #[arg(long)]
        cycle: Option<String>,
        /// Preview the exact requested resources and configuration price
        #[arg(long, conflicts_with = "wait")]
        dry_run: bool,
        /// Accept the quoted purchase
        #[arg(long, short)]
        yes: bool,
        /// Wait for the new server to become active
        #[arg(long)]
        wait: bool,
        /// Wait deadline in seconds
        #[arg(long, default_value_t = 900, requires = "wait")]
        timeout: u64,
    },
}

pub async fn snapshots(ctx: &Ctx, cmd: &SnapshotsCommand) -> Result<()> {
    let api = ctx.api()?;
    match cmd {
        SnapshotsCommand::List { server } => {
            let req = match server {
                Some(s) => ops::snapshots(s),
                None => ops::account_snapshots(),
            };
            let resp = api.send(req).await?;
            let cols: Vec<Col> = if server.is_some() {
                SNAPSHOT_COLS.to_vec()
            } else {
                let mut c = SNAPSHOT_COLS.to_vec();
                c[2] = col("SERVER", "/sourceHostname");
                c
            };
            print_list(ctx.format, &resp, &items(&resp), &cols, "No snapshots.");
        }
        SnapshotsCommand::Create { server, name } => {
            let resp = api.send(ops::create_snapshot(server, name)).await?;
            print_object(ctx.format, &resp, SNAPSHOT_COLS);
        }
        SnapshotsCommand::Delete {
            server,
            snapshot,
            yes,
        } => {
            confirm(*yes, &format!("This deletes snapshot {snapshot}"), snapshot)?;
            api.send(ops::delete_snapshot(server, snapshot)).await?;
            done_or_json(
                ctx,
                json!({ "deleted": snapshot }),
                format!("Snapshot {snapshot} deleted."),
            );
        }
        SnapshotsCommand::Restore {
            server,
            snapshot,
            yes,
        } => {
            confirm(
                *yes,
                &format!("Restoring overwrites server {server} with snapshot {snapshot}"),
                server,
            )?;
            let resp = api.send(ops::restore_snapshot(server, snapshot)).await?;
            done_or_json(ctx, resp, format!("Restoring {server} from {snapshot}."));
        }
        SnapshotsCommand::Spawn {
            server,
            snapshot,
            hostname,
            plan,
            cycle,
            dry_run,
            yes,
            wait,
            timeout,
        } => {
            let (request, preview) = prepare_spawn(
                &api,
                server,
                snapshot,
                hostname.as_deref(),
                plan.as_deref(),
                cycle.as_deref(),
            )
            .await?;
            if *dry_run {
                super::catalog::print_quote(ctx, &preview);
                return Ok(());
            }
            eprintln!("Snapshot spawn: CPU {}, RAM {} MB, disk {} GB, cycle {}. Price: {}. The backend revalidates resources and pricing at purchase.",
                preview["cpu"], preview["ramMb"], preview["diskGb"], preview["billingCycle"], super::catalog::quote_summary(&preview["quote"]));
            confirm(
                *yes,
                "Create a paid server from this snapshot using the displayed configuration price",
                snapshot,
            )?;
            let resp = api.send(request).await?;
            let new_id = resp["newServiceId"].as_str().or_else(|| resp["id"].as_str())
                .ok_or_else(|| CliError::Other("spawn succeeded but returned no server id; inspect `zy servers list` before retrying".into()))?;
            eprintln!("Server {new_id} is provisioning; follow it with `zy servers wait {new_id} --timeout {timeout}` and `zy servers get {new_id}` for applied resources and prices.");
            if *wait {
                let server = super::servers::wait_for_state(
                    &api,
                    new_id,
                    "active",
                    std::time::Duration::from_secs(*timeout),
                    !ctx.json(),
                )
                .await?;
                print_object(ctx.format, &server, super::servers::DETAIL_COLS);
                return Ok(());
            }
            print_object(
                ctx.format,
                &resp,
                &[
                    col("New server", "/newServiceId"),
                    col("Hostname", "/hostname"),
                    col("From snapshot", "/sourceSnapshotId"),
                ],
            );
        }
    }
    Ok(())
}

fn done_or_json(ctx: &Ctx, value: Value, note: String) {
    if ctx.json() {
        print_json(&value);
    } else {
        ctx.done(note);
    }
}

// ─── SSH keys ───────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum SshKeysCommand {
    /// List saved SSH keys
    #[command(visible_alias = "ls")]
    List,
    /// Save a public key
    Add {
        name: String,
        /// Public key file, e.g. ~/.ssh/id_ed25519.pub
        #[arg(long, value_name = "FILE", conflicts_with = "key")]
        file: Option<std::path::PathBuf>,
        /// The public key itself
        #[arg(long)]
        key: Option<String>,
    },
    /// Delete a saved key
    #[command(visible_alias = "rm")]
    Delete { id: String },
}

pub async fn ssh_keys(ctx: &Ctx, cmd: &SshKeysCommand) -> Result<()> {
    let cols = [
        col("ID", "/id"),
        col("NAME", "/name"),
        col("FINGERPRINT", "/fingerprint"),
        col("CREATED", "/createdAt"),
    ];
    match cmd {
        SshKeysCommand::List => {
            let resp = ctx.send(ops::ssh_keys()).await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                &cols,
                "No SSH keys. Add one with `zy ssh-keys add NAME --file ~/.ssh/id_ed25519.pub`.",
            );
        }
        SshKeysCommand::Add { name, file, key } => {
            let public_key = match (file, key) {
                (Some(f), _) => std::fs::read_to_string(f)
                    .map_err(|e| CliError::Usage(format!("cannot read {}: {e}", f.display())))?,
                (None, Some(k)) => k.clone(),
                (None, None) => return Err(CliError::Usage("give --file or --key".into())),
            };
            let public_key = public_key.trim();
            if public_key.contains("PRIVATE KEY") {
                return Err(CliError::Usage(
                    "that is a private key — give the .pub file".into(),
                ));
            }
            let resp = ctx.send(ops::add_ssh_key(name, public_key)).await?;
            print_object(ctx.format, &resp, &cols);
        }
        SshKeysCommand::Delete { id } => {
            ctx.send(ops::delete_ssh_key(id)).await?;
            done_or_json(
                ctx,
                json!({ "deleted": id }),
                format!("SSH key {id} deleted."),
            );
        }
    }
    Ok(())
}

// ─── Reserved IPs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Family {
    Ipv4,
    Ipv6,
}

impl Family {
    fn api(self) -> &'static str {
        match self {
            Family::Ipv4 => "ipv4",
            Family::Ipv6 => "ipv6",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ReservedIpsCommand {
    /// List reserved IPs
    #[command(visible_alias = "ls")]
    List,
    /// Reserve IPs: published rate 2.50 USD/IP/month, non-refundable, auto-renew enabled
    Create {
        #[arg(long)]
        region: String,
        #[arg(long, value_enum)]
        family: Option<Family>,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=30))]
        count: u32,
        /// Show the server quote without purchasing (published tariff on older servers)
        #[arg(long)]
        dry_run: bool,
        /// Accept the non-refundable charge and automatic monthly renewal
        #[arg(long, short)]
        yes: bool,
    },
    /// Attach a reserved IP to a server
    Attach { id: String, server: String },
    /// Detach a reserved IP from its server (billing continues)
    Detach { id: String },
    /// Turn monthly auto-renew on or off
    AutoRenew {
        id: String,
        /// on or off
        #[arg(action = clap::ArgAction::Set, value_parser = clap::builder::BoolishValueParser::new(), value_name = "on|off")]
        enabled: bool,
    },
    /// Release a reserved IP back to the pool (non-refundable)
    Release {
        id: String,
        #[arg(long, short)]
        yes: bool,
    },
}

pub async fn reserved_ips(ctx: &Ctx, cmd: &ReservedIpsCommand) -> Result<()> {
    let cols = [
        col("ID", "/id"),
        col("IP", "/ip"),
        col("FAMILY", "/family"),
        col("REGION", "/region"),
        col("STATUS", "/status"),
        col("SERVER", "/attachedServiceId"),
        col("AUTO-RENEW", "/autoRenew"),
        col("NEXT BILL", "/nextBillAt"),
    ];
    match cmd {
        ReservedIpsCommand::List => {
            let resp = ctx.send(ops::reserved_ips()).await?;
            print_list(ctx.format, &resp, &items(&resp), &cols, "No reserved IPs.");
        }
        ReservedIpsCommand::Create {
            region,
            family,
            count,
            dry_run,
            yes,
        } => {
            let api = ctx.api()?;
            let preview = reservation_quote(&api, region, family.map(Family::api), *count).await?;
            let total = preview["totalMonthlyCents"]
                .as_i64()
                .expect("validated quote");
            let currency = preview["currency"].as_str().expect("validated currency");
            if *dry_run {
                print_object(ctx.format, &preview, &[]);
                return Ok(());
            }
            confirm(*yes, &format!("Reserve {count} IP(s) in {region}: {} per month, non-refundable, auto-renew enabled ({})", money(Some(total), Some(currency)), preview["priceSource"].as_str().unwrap_or("server quote")), region)?;
            let resp = api
                .send(quoted_reservation(
                    region,
                    family.map(Family::api),
                    *count,
                    &preview,
                ))
                .await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                &cols,
                "Nothing was reserved.",
            );
            if let Some(charged) = resp["receipt"]["chargedAmountCents"].as_i64() {
                eprintln!("Server receipt: {} charged; status {}. Next billing dates and auto-renew settings are in the IP receipt.", money(Some(charged), resp["receipt"]["currency"].as_str()), resp["receipt"]["chargeStatus"].as_str().unwrap_or("unknown"));
            } else {
                eprintln!("Charged amount is unconfirmed (receipt status: {}). Check `zy billing ledger` before retrying the purchase.", resp["receipt"]["chargeStatus"].as_str().unwrap_or("unavailable"));
            }
        }
        ReservedIpsCommand::Attach { id, server } => {
            let resp = ctx.send(ops::attach_reserved_ip(id, server)).await?;
            done_or_json(ctx, resp, format!("Reserved IP {id} attached to {server}."));
        }
        ReservedIpsCommand::Detach { id } => {
            let resp = ctx.send(ops::detach_reserved_ip(id)).await?;
            done_or_json(ctx, resp, format!("Reserved IP {id} detached."));
        }
        ReservedIpsCommand::AutoRenew { id, enabled } => {
            let resp = ctx.send(ops::reserved_ip_auto_renew(id, *enabled)).await?;
            done_or_json(
                ctx,
                resp,
                format!(
                    "Auto-renew {} for {id}.",
                    if *enabled { "on" } else { "off" }
                ),
            );
        }
        ReservedIpsCommand::Release { id, yes } => {
            confirm(
                *yes,
                &format!("Releasing {id} gives the address up for good"),
                id,
            )?;
            ctx.send(ops::release_reserved_ip(id)).await?;
            done_or_json(
                ctx,
                json!({ "released": id }),
                format!("Reserved IP {id} released."),
            );
        }
    }
    Ok(())
}

// ─── Server IPs ─────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum IpsCommand {
    /// List a server's public IPs
    #[command(visible_alias = "ls")]
    List { server: String },
    /// Attach a reserved IPv4 address, or allocate IPv6 where available
    Add {
        server: String,
        #[arg(long, value_enum, conflicts_with = "reserved")]
        family: Option<Family>,
        /// A reserved IP address to attach
        #[arg(long, value_name = "IP")]
        reserved: Option<String>,
    },
    /// Detach an extra IP from a server
    #[command(visible_alias = "rm")]
    Remove {
        server: String,
        ip: String,
        #[arg(long, short)]
        yes: bool,
    },
}

pub async fn ips(ctx: &Ctx, cmd: &IpsCommand) -> Result<()> {
    let cols = [
        col("IP", "/ip"),
        col("FAMILY", "/family"),
        col("PRIMARY", "/primary"),
        col("RESERVED", "/reserved"),
        col("STATUS", "/status"),
        col("GATEWAY", "/gateway"),
        col("CIDR", "/cidr"),
    ];
    match cmd {
        IpsCommand::List { server } => {
            let resp = ctx.send(ops::server_ips(server)).await?;
            let rows = resp.get("addresses").map(items).unwrap_or_default();
            print_list(ctx.format, &resp, &rows, &cols, "No addresses.");
        }
        IpsCommand::Add {
            server,
            family,
            reserved,
        } => {
            let api = ctx.api()?;
            ops::preflight_ip(&api, server, family.map(Family::api), reserved.as_deref()).await?;
            let resp = api
                .send(ops::attach_ip(
                    server,
                    family.map(Family::api),
                    reserved.as_deref(),
                ))
                .await?;
            print_object(ctx.format, &resp, &[]);
        }
        IpsCommand::Remove { server, ip, yes } => {
            confirm(*yes, &format!("This detaches {ip} from {server}"), ip)?;
            ctx.send(ops::detach_ip(server, ip)).await?;
            done_or_json(
                ctx,
                json!({ "detached": ip }),
                format!("{ip} detached from {server}."),
            );
        }
    }
    Ok(())
}

// ─── Firewall ───────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum FirewallCommand {
    /// List a server's firewall rules
    #[command(visible_alias = "ls")]
    List { server: String },
    /// Add a rule
    Add {
        server: String,
        /// inbound or outbound (in/out aliases are accepted)
        #[arg(long, default_value = "inbound", value_parser = ["inbound", "outbound", "in", "out"])]
        direction: String,
        /// tcp, udp, icmp or any
        #[arg(long, default_value = "tcp")]
        protocol: String,
        /// Port or range, e.g. 22 or 8000-8100
        #[arg(long, default_value = "")]
        port: String,
        /// Source CIDR
        #[arg(long, default_value = "0.0.0.0/0")]
        source: String,
        /// allow or deny
        #[arg(long, default_value = "allow")]
        action: String,
    },
    /// Delete a rule
    #[command(visible_alias = "rm")]
    Delete { server: String, rule: String },
}

pub async fn firewall(ctx: &Ctx, cmd: &FirewallCommand) -> Result<()> {
    let cols = [
        col("ID", "/id"),
        col("DIRECTION", "/direction"),
        col("PROTOCOL", "/protocol"),
        col("PORT", "/port"),
        col("SOURCE", "/source"),
        col("ACTION", "/action"),
        col("ENABLED", "/enabled"),
    ];
    match cmd {
        FirewallCommand::List { server } => {
            let resp = ctx.send(ops::firewall_rules(server)).await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                &cols,
                "No firewall rules.",
            );
        }
        FirewallCommand::Add {
            server,
            direction,
            protocol,
            port,
            source,
            action,
        } => {
            let resp = ctx
                .send(ops::add_firewall_rule(
                    server, direction, protocol, port, source, action,
                ))
                .await?;
            print_object(ctx.format, &resp, &cols);
        }
        FirewallCommand::Delete { server, rule } => {
            ctx.send(ops::delete_firewall_rule(server, rule)).await?;
            done_or_json(
                ctx,
                json!({ "deleted": rule }),
                format!("Firewall rule {rule} deleted."),
            );
        }
    }
    Ok(())
}

// ─── Account ────────────────────────────────────────────────────────────

pub async fn whoami(ctx: &Ctx) -> Result<()> {
    let api = ctx.api()?;
    let me = api.send(ops::whoami()).await?;
    if ctx.json() {
        print_json(&me);
        return Ok(());
    }
    print_object(
        ctx.format,
        &me,
        &[
            col("Email", "/email"),
            col("Name", "/name"),
            col("User ID", "/id"),
            col("Email verified", "/emailVerified"),
            col("MFA", "/mfaEnabled"),
        ],
    );
    eprintln!(
        "Signed in to {} with a {}.",
        ctx.settings.url,
        api.credential().describe()
    );
    Ok(())
}

#[derive(Subcommand, Debug)]
pub enum BillingCommand {
    /// Wallet balance
    Balance,
    /// Wallet ledger
    Ledger {
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long, default_value_t = 50)]
        per_page: u32,
    },
    /// Invoices
    Invoices {
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long, default_value_t = 20)]
        per_page: u32,
    },
}

pub async fn billing(ctx: &Ctx, cmd: &BillingCommand) -> Result<()> {
    match cmd {
        BillingCommand::Balance => {
            let resp = ctx.send(ops::balance()).await?;
            if ctx.json() {
                print_json(&resp);
            } else {
                let currency = resp.get("currency").and_then(Value::as_str);
                println!(
                    "Balance:     {}",
                    money(resp.get("balance").and_then(Value::as_i64), currency)
                );
                println!(
                    "Refundable:  {}",
                    money(
                        resp.get("refundableBalance").and_then(Value::as_i64),
                        currency
                    )
                );
            }
        }
        BillingCommand::Ledger { page, per_page } => {
            let resp = ctx.send(ops::ledger(*page, *per_page)).await?;
            let rows: Vec<Value> = items(&resp)
                .into_iter()
                .map(|mut r| {
                    let cur = r
                        .get("currency")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    r["_amount"] = json!(precise_money(
                        r.get("amountMicro").and_then(Value::as_i64),
                        r.get("amountCents").and_then(Value::as_i64),
                        cur.as_deref()
                    ));
                    r["_balance"] = json!(precise_money(
                        r.get("balanceMicro").and_then(Value::as_i64),
                        r.get("balanceCents").and_then(Value::as_i64),
                        cur.as_deref()
                    ));
                    r
                })
                .collect();
            print_list(
                ctx.format,
                &resp,
                &rows,
                &[
                    col("WHEN", "/createdAt"),
                    col("TYPE", "/type"),
                    col("AMOUNT", "/_amount"),
                    col("BALANCE", "/_balance"),
                    col("DESCRIPTION", "/description"),
                ],
                "No ledger entries.",
            );
            page_note(ctx, &resp);
        }
        BillingCommand::Invoices { page, per_page } => {
            let resp = ctx.send(ops::invoices(*page, *per_page)).await?;
            let rows: Vec<Value> = items(&resp)
                .into_iter()
                .map(|mut r| {
                    let cur = r
                        .get("currency")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    r["_total"] = json!(precise_money(
                        r.get("totalMicroCents").and_then(Value::as_i64),
                        r.get("totalCents").and_then(Value::as_i64),
                        cur.as_deref()
                    ));
                    r
                })
                .collect();
            print_list(
                ctx.format,
                &resp,
                &rows,
                &[
                    col("ID", "/id"),
                    col("NUMBER", "/number"),
                    col("STATUS", "/status"),
                    col("TOTAL", "/_total"),
                    col("DUE", "/dueDate"),
                    col("PAID", "/paidAt"),
                ],
                "No invoices.",
            );
            page_note(ctx, &resp);
        }
    }
    Ok(())
}

fn page_note(ctx: &Ctx, resp: &Value) {
    if ctx.json() {
        return;
    }
    if let (Some(page), Some(pages)) = (
        resp.get("page").and_then(Value::as_u64),
        resp.get("totalPages").and_then(Value::as_u64),
    ) {
        if pages > 1 {
            eprintln!("Page {page} of {pages} — use --page to see more.");
        }
    }
}

/// Supply resources explicitly so a chosen catalog plan never silently inherits
/// source CPU/RAM overrides. The source disk is a conservative clone-size floor.
pub async fn prepare_spawn(
    api: &crate::api::ApiClient,
    server: &str,
    snapshot: &str,
    hostname: Option<&str>,
    selected_plan: Option<&str>,
    cycle: Option<&str>,
) -> Result<(crate::api::Request, Value)> {
    let source = api.send(ops::server(server)).await?;
    let source_disk = source["diskGb"].as_u64().ok_or_else(|| {
        CliError::Usage("source disk size is missing; cannot quote a snapshot clone".into())
    })?;
    let source_plan = source["planId"]
        .as_str()
        .or_else(|| source["plan"]["id"].as_str())
        .ok_or_else(|| CliError::Usage("source server has no catalog plan id".into()))?;
    let plan_id = match selected_plan {
        Some(plan) => super::servers::resolve_plan(api, plan).await?,
        None => source_plan.to_string(),
    };
    let catalog = api.send(ops::pricing_catalog()).await?;
    let plan = items(&catalog["plans"])
        .into_iter()
        .find(|p| p["id"].as_str() == Some(&plan_id))
        .ok_or_else(|| CliError::Usage("spawn plan is absent from the catalog".into()))?;
    let mut requested = source.clone();
    if selected_plan.is_some() {
        requested["cpu"] = plan["cpuCores"].clone();
        requested["ramMb"] = plan["memoryMb"].clone();
        requested["diskGb"] = json!(source_disk.max(plan["diskGb"].as_u64().unwrap_or(0)));
        requested["bandwidthTb"] = plan["bandwidthTb"].clone();
    }
    let cycle = cycle
        .or_else(|| source["billingCycle"].as_str())
        .ok_or_else(|| {
            CliError::Usage("source server billing cycle is missing; pass --cycle".into())
        })?;
    requested["billingCycle"] = json!(cycle);
    // Spawn constructs a new config without these source add-ons.
    requested["nested"] = json!(false);
    requested["autoBackups"] = json!(false);
    for (field, min, max) in [("cpu", 1, 32), ("ramMb", 512, 131072), ("diskGb", 1, 5000)] {
        if requested[field].as_u64().is_none_or(|n| n < min || n > max) {
            return Err(CliError::Usage(format!("snapshot spawn requires {field} between {min} and {max}; cannot quote a configuration the backend would clamp")));
        }
    }
    let quote_body = super::servers::existing_quote_body(&requested, &plan)?;
    let selected = super::catalog::selected_plans(&catalog, source["region"].as_str(), cycle);
    if items(&selected["plans"])
        .iter()
        .any(|p| p["id"].as_str() == Some(&plan_id) && p["price"]["inStock"] == false)
    {
        return Err(CliError::Usage(
            "selected spawn plan is out of stock in the source region".into(),
        ));
    }
    let quote = api.send(ops::pricing_quote(quote_body)).await?;
    super::catalog::validate_quote(&quote)?;
    let mut request = ops::spawn_snapshot(server, snapshot, hostname, Some(&plan_id));
    let body = request.body.as_mut().expect("spawn body");
    for (key, resource) in [
        ("cpuCores", "cpu"),
        ("ramMb", "ramMb"),
        ("diskGb", "diskGb"),
        ("transferTb", "bandwidthTb"),
    ] {
        if let Some(value) = requested[resource].as_u64() {
            body[key] = json!(value);
        }
    }
    body["billingCycle"] = json!(cycle);
    let preview = json!({"planId": plan_id, "region": source["region"], "cpu": requested["cpu"],
        "ramMb": requested["ramMb"], "diskGb": requested["diskGb"], "bandwidthTb": requested["bandwidthTb"],
        "billingCycle": cycle, "quote": quote, "includeIpv4": source["ipAddress"].as_str().is_some_and(|ip| !ip.is_empty()),
        "availability": "catalog only; live capacity is validated at spawn", "diskPolicy": "at least the current source disk; clone disks cannot shrink"});
    Ok((request, preview))
}

/// Shared with MCP. Only an absent route permits a published-price fallback;
/// quota, stock, authentication and backend failures must stop the purchase.
pub async fn reservation_quote(
    api: &crate::api::ApiClient,
    region: &str,
    family: Option<&str>,
    count: u32,
) -> Result<Value> {
    let family = family.unwrap_or("ipv4");
    if !(1..=30).contains(&count) || !matches!(family, "ipv4" | "ipv6") {
        return Err(CliError::Usage(
            "count must be 1–30 and family must be ipv4 or ipv6".into(),
        ));
    }
    let mut quote = match api
        .send(ops::quote_reserved_ips(region, family, count))
        .await
    {
        Ok(value) => value,
        Err(err) if err.is_missing_route() => {
            return Ok(json!({"region": region, "family": family,
            "count": count, "currency": "USD", "unitMonthlyCents": 250, "totalMonthlyCents": i64::from(count) * 250,
            "billingCycle": "monthly", "refundable": false, "autoRenew": true,
            "advisory": true, "priceSource": "published fixed tariff; not a live server quote (endpoint unavailable)"}))
        }
        Err(err) => return Err(err.into()),
    };
    let unit = quote["unitMonthlyCents"].as_i64();
    if quote["region"].as_str() != Some(region)
        || quote["family"].as_str() != Some(family)
        || quote["count"].as_u64() != Some(u64::from(count))
        || quote["totalMonthlyCents"].as_i64().is_none_or(|n| n < 0)
        || unit.is_none_or(|n| n < 0)
        || unit.and_then(|n| n.checked_mul(i64::from(count))) != quote["totalMonthlyCents"].as_i64()
        || quote["currency"].as_str().is_none_or(|s| s.is_empty())
        || quote["refundable"] != false
        || quote["autoRenew"] != true
        || quote["billingCycle"] != "monthly"
    {
        return Err(CliError::Other("reserved-IP quote omitted valid price or purchase policy fields; nothing was purchased".into()));
    }
    quote["priceSource"] = json!("server quote");
    Ok(quote)
}

pub fn quoted_reservation(
    region: &str,
    family: Option<&str>,
    count: u32,
    quote: &Value,
) -> crate::api::Request {
    let mut request = ops::reserve_ips(region, family, Some(count));
    if quote["priceSource"] == "server quote" {
        if let Some(body) = request.body.as_mut() {
            body["expectedTotalCents"] = quote["totalMonthlyCents"].clone();
            body["expectedCurrency"] = quote["currency"].clone();
        }
    }
    request
}
