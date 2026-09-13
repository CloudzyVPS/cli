//! `zy regions`, `zy plans`, `zy os`, `zy apps`.

use clap::Subcommand;
use serde_json::{json, Value};

use super::context::{money, Ctx};
use crate::api::{items, ops};
use crate::error::Result;
use crate::output::{col, print_json, print_list, print_object};

#[derive(Subcommand, Debug)]
pub enum RegionsCommand {
    /// List regions
    #[command(visible_alias = "ls")]
    List,
}

#[derive(Subcommand, Debug)]
pub enum PlansCommand {
    /// List plans, with prices for a region
    #[command(visible_alias = "ls")]
    List {
        /// Region id to price for (base prices when omitted)
        #[arg(long)]
        region: Option<String>,
        /// Billing cycle to price
        #[arg(long, default_value = "monthly")]
        cycle: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum OsCommand {
    /// List installable OS templates
    #[command(visible_alias = "ls")]
    List,
}

#[derive(Subcommand, Debug)]
pub enum AppsCommand {
    /// List one-click apps
    #[command(visible_alias = "ls")]
    List,
    /// Show a one-click app and its parameters
    Get { name: String },
}

pub async fn regions(ctx: &Ctx, _cmd: &RegionsCommand) -> Result<()> {
    let resp = ctx.send(ops::regions()).await?;
    let rows: Vec<Value> = items(&resp)
        .into_iter()
        .filter(|r| r.get("isHidden") != Some(&json!(true)))
        .collect();
    print_list(
        ctx.format,
        &resp,
        &rows,
        &[
            col("ID", "/id"),
            col("NAME", "/name"),
            col("CODE", "/abbr"),
            col("ACTIVE", "/isActive"),
            col("OUT OF STOCK", "/isOutOfStock"),
            col("PREMIUM", "/isPremium"),
        ],
        "No regions.",
    );
    Ok(())
}

pub async fn plans(ctx: &Ctx, cmd: &PlansCommand) -> Result<()> {
    let PlansCommand::List { region, cycle } = cmd;
    let catalog = ctx.send(ops::pricing_catalog()).await?;
    if ctx.json() {
        print_json(&catalog);
        return Ok(());
    }
    let prices = catalog.get("prices").map(items).unwrap_or_default();
    let price_for = |plan_id: &str| -> String {
        let matches = |p: &&Value, rid: &str| {
            p.get("planId").and_then(Value::as_str) == Some(plan_id)
                && p.get("billingCycle").and_then(Value::as_str) == Some(cycle.as_str())
                && p.get("regionId").and_then(Value::as_str).unwrap_or("") == rid
        };
        let chosen = region
            .as_deref()
            .and_then(|r| prices.iter().find(|p| matches(p, r)))
            .or_else(|| prices.iter().find(|p| matches(p, "")));
        match chosen {
            Some(p) => format!(
                "{}/mo",
                money(
                    p.get("monthlyEquivCents").and_then(Value::as_i64),
                    p.get("currency").and_then(Value::as_str)
                )
            ),
            None => "-".into(),
        }
    };
    let rows: Vec<Value> = catalog
        .get("plans")
        .map(items)
        .unwrap_or_default()
        .into_iter()
        .map(|mut p| {
            let id = p
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            p["_price"] = json!(price_for(&id));
            p
        })
        .collect();
    print_list(
        ctx.format,
        &catalog,
        &rows,
        &[
            col("ID", "/id"),
            col("SLUG", "/slug"),
            col("NAME", "/name"),
            col("TYPE", "/instanceType"),
            col("VCPU", "/cpuCores"),
            col("RAM MB", "/memoryMb"),
            col("DISK GB", "/diskGb"),
            col("BW TB", "/bandwidthTb"),
            col("PRICE", "/_price"),
        ],
        "No plans.",
    );
    Ok(())
}

pub async fn os(ctx: &Ctx, _cmd: &OsCommand) -> Result<()> {
    let resp = ctx.send(ops::os_templates()).await?;
    let rows: Vec<Value> = items(&resp)
        .into_iter()
        .filter(|o| o.get("available") != Some(&json!(false)))
        .collect();
    print_list(
        ctx.format,
        &resp,
        &rows,
        &[
            col("ID", "/id"),
            col("NAME", "/name"),
            col("FAMILY", "/family"),
            col("VERSION", "/version"),
            col("MIN RAM MB", "/minRamMb"),
            col("MIN DISK GB", "/minDiskGb"),
        ],
        "No OS templates.",
    );
    Ok(())
}

pub async fn apps(ctx: &Ctx, cmd: &AppsCommand) -> Result<()> {
    match cmd {
        AppsCommand::List => {
            let resp = ctx.send(ops::apps()).await?;
            let rows: Vec<Value> = items(&resp)
                .into_iter()
                .filter(|a| a.get("enabled") != Some(&json!(false)))
                .collect();
            print_list(
                ctx.format,
                &resp,
                &rows,
                &[
                    col("NAME", "/name"),
                    col("TITLE", "/displayName"),
                    col("CATEGORY", "/category"),
                    col("MIN RAM MB", "/minRamMb"),
                    col("MIN VCPU", "/minVcpu"),
                    col("OS", "/supportedOsIds"),
                ],
                "No one-click apps.",
            );
        }
        AppsCommand::Get { name } => {
            let resp = ctx.send(ops::app(name)).await?;
            let app = resp.get("data").cloned().unwrap_or(resp);
            print_object(
                ctx.format,
                &app,
                &[
                    col("Name", "/name"),
                    col("Title", "/displayName"),
                    col("Category", "/category"),
                    col("Description", "/description"),
                    col("Min RAM (MB)", "/minRamMb"),
                    col("Min vCPU", "/minVcpu"),
                    col("OS", "/supportedOsIds"),
                    col("Parameters", "/manifest/params"),
                ],
            );
        }
    }
    Ok(())
}
