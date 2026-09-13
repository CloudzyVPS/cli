//! `zy snapshots`, `zy ssh-keys`, `zy reserved-ips`, `zy ips`, `zy firewall`,
//! `zy whoami`, `zy billing`.

use clap::{Subcommand, ValueEnum};
use serde_json::{json, Value};

use super::context::{confirm, money, Ctx};
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
        /// Plan for the new server (defaults to the source server's plan)
        #[arg(long)]
        plan: Option<String>,
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
        } => {
            let resp = api
                .send(ops::spawn_snapshot(
                    server,
                    snapshot,
                    hostname.as_deref(),
                    plan.as_deref(),
                ))
                .await?;
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
    /// Reserve new IPs in a region (billed monthly)
    Create {
        #[arg(long)]
        region: String,
        #[arg(long, value_enum)]
        family: Option<Family>,
        #[arg(long, default_value_t = 1)]
        count: u32,
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
        } => {
            let resp = ctx
                .send(ops::reserve_ips(
                    region,
                    family.map(Family::api),
                    Some(*count),
                ))
                .await?;
            print_list(
                ctx.format,
                &resp,
                &items(&resp),
                &cols,
                "Nothing was reserved.",
            );
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
    /// Attach an extra IP from the pool, or one of your reserved IPs
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
            let resp = ctx
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
        /// in or out
        #[arg(long, default_value = "in")]
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
                    r["_amount"] = json!(money(
                        r.get("amountCents").and_then(Value::as_i64),
                        cur.as_deref()
                    ));
                    r["_balance"] = json!(money(
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
                    r["_total"] = json!(money(
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
