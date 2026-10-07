//! `zy regions`, `zy plans`, `zy os`, `zy apps`.

use clap::Subcommand;
use serde_json::{json, Value};

use super::context::{money, Ctx};
use crate::api::{items, ops, ApiClient};
use crate::error::{CliError, Result};
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
    /// Quote a configuration including mandatory IPv4 charges (no purchase)
    Quote {
        #[arg(long)]
        plan: String,
        #[arg(long)]
        region: String,
        #[arg(long, default_value = "monthly")]
        cycle: String,
        /// Exclude IPv4 charges for an IPv6-only configuration
        #[arg(long)]
        ipv6_only: bool,
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
    match cmd {
        PlansCommand::Quote {
            plan,
            region,
            cycle,
            ipv6_only,
        } => {
            let api = ctx.api()?;
            let plan_id = super::servers::resolve_plan(&api, plan).await?;
            let quote = quote_plan(&api, &plan_id, region, cycle, !ipv6_only).await?;
            print_quote(ctx, &quote);
        }
        PlansCommand::List { region, cycle } => {
            let catalog = ctx.send(ops::pricing_catalog()).await?;
            let selected = selected_plans(&catalog, region.as_deref(), cycle);
            let rows = items(&selected["plans"]);
            if !ctx.json() {
                eprintln!("Catalog base prices exclude configuration extras. Use `zy plans quote --plan SLUG --region REGION --cycle {cycle}` for IPv4-inclusive pricing. Catalog stock does not guarantee live capacity.");
            }
            print_list(
                ctx.format,
                &selected,
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
                    col("BASE PRICE", "/_price"),
                    col("CATALOG STOCK", "/price/inStock"),
                ],
                "No plans.",
            );
        }
    }
    Ok(())
}

pub fn selected_plans(catalog: &Value, region: Option<&str>, cycle: &str) -> Value {
    let prices = catalog.get("prices").map(items).unwrap_or_default();
    let rows: Vec<_> = catalog
        .get("plans")
        .map(items)
        .unwrap_or_default()
        .into_iter()
        .map(|mut plan| {
            let pick = |rid: &str| {
                prices.iter().find(|p| {
                    p["planId"] == plan["id"]
                        && p["billingCycle"].as_str() == Some(cycle)
                        && p["regionId"].as_str().unwrap_or("") == rid
                })
            };
            let price = region
                .and_then(pick)
                .or_else(|| pick(""))
                .cloned()
                .unwrap_or(Value::Null);
            plan["_price"] = json!(if cycle == "hourly" {
                price["monthlyEquivCents"]
                    .as_f64()
                    .map(|v| {
                        format!(
                            "{:.8} {}/hr",
                            v / 100.0 / 672.0,
                            price["currency"].as_str().unwrap_or("USD")
                        )
                    })
                    .unwrap_or("-".into())
            } else {
                format!(
                    "{}/mo equivalent",
                    money(
                        price["monthlyEquivCents"].as_i64(),
                        price["currency"].as_str()
                    )
                )
            });
            plan["price"] = price;
            plan
        })
        .collect();
    json!({"region": region, "billingCycle": cycle, "priceScope": "catalog base; excludes configuration extras", "availabilityScope": "catalog stock; live capacity is validated at create", "plans": rows})
}

pub async fn quote_plan(
    api: &ApiClient,
    plan: &str,
    region: &str,
    cycle: &str,
    ipv4: bool,
) -> Result<Value> {
    let catalog = api.send(ops::pricing_catalog()).await?;
    let selected = selected_plans(&catalog, Some(region), cycle);
    let entry = items(&selected["plans"])
        .into_iter()
        .find(|p| p["id"].as_str() == Some(plan))
        .ok_or_else(|| CliError::Usage(format!("unknown plan {plan}; see `zy plans list`")))?;
    if entry["price"]["inStock"] == false {
        return Err(CliError::Usage(format!(
            "plan {plan} is out of stock in {region}; see `zy plans list --region {region}`"
        )));
    }
    let quote = api
        .send(ops::pricing_quote(json!({"planId": plan, "region": region,
        "billingCycle": cycle, "quantity": 1, "includeIpv4": ipv4, "extras": {}})))
        .await?;
    validate_quote(&quote)?;
    let availability = match api.send(ops::plan_capacity(plan, region)).await {
        Ok(value) => {
            if value["available"].as_bool().is_none() || value["capacityKnown"].as_bool().is_none()
            {
                return Err(CliError::Other(
                    "capacity response omitted its availability fields; nothing was purchased"
                        .into(),
                ));
            }
            value
        }
        Err(err) if err.is_missing_route() => json!({"available": null, "capacityKnown": false,
            "advisory": true, "reason": "preflight endpoint unavailable; catalog stock only"}),
        Err(err) => return Err(err.into()),
    };
    Ok(
        json!({"planId": plan, "region": region, "billingCycle": cycle, "includeIpv4": ipv4,
        "cpu": entry["cpuCores"], "ramMb": entry["memoryMb"], "diskGb": entry["diskGb"],
        "quote": quote, "availability": availability}),
    )
}

pub fn validate_quote(quote: &Value) -> Result<()> {
    if quote["subtotalMonthlyCents"].as_i64().is_none_or(|n| n < 0)
        || quote["currency"].as_str().is_none_or(|s| s.is_empty())
    {
        return Err(CliError::Other(
            "pricing response omitted a valid monthly amount or currency; nothing was purchased"
                .into(),
        ));
    }
    Ok(())
}

pub fn print_quote(ctx: &Ctx, quote: &Value) {
    let mut display = quote.clone();
    let q = &quote["quote"];
    display["_monthly"] = json!(money(
        q["subtotalMonthlyCents"].as_i64(),
        q["currency"].as_str()
    ));
    display["_hourly"] = json!(q["subtotalMonthlyCents"]
        .as_f64()
        .map(|v| format!(
            "{:.8} {}/hr",
            v / 100.0 / 672.0,
            q["currency"].as_str().unwrap_or("USD")
        ))
        .unwrap_or("-".into()));
    display["_total"] = json!(money(q["grandTotalCents"].as_i64(), q["currency"].as_str()));
    if ctx.json() {
        print_json(quote);
    } else {
        print_object(
            ctx.format,
            &display,
            &[
                col("Plan", "/planId"),
                col("Region", "/region"),
                col("vCPU", "/cpu"),
                col("RAM MB", "/ramMb"),
                col("Disk GB", "/diskGb"),
                col("Cycle", "/billingCycle"),
                col("IPv4", "/includeIpv4"),
                col("Monthly equivalent", "/_monthly"),
                col("Hourly equivalent", "/_hourly"),
                col("Quoted cycle total (rounded)", "/_total"),
                col("Applied features", "/quote/appliedFeatures"),
                col("Availability", "/availability"),
            ],
        );
    }
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

pub fn quote_summary(quote: &Value) -> String {
    let currency = quote["currency"].as_str().unwrap_or("USD");
    let monthly = quote["subtotalMonthlyCents"].as_i64();
    let hourly = monthly
        .map(|c| format!("{:.8} {currency}/hr", c as f64 / 100.0 / 672.0))
        .unwrap_or("unavailable".into());
    format!(
        "{} /month equivalent; {hourly}",
        money(monthly, Some(currency))
    )
}

/// A price preview may describe a full region, but a purchase must refuse it.
pub fn require_capacity(quote: &Value) -> Result<()> {
    if quote["availability"]["available"] == false {
        return Err(crate::api::ApiError::Http { status: 409,
            code: Some(quote["availability"]["reason"].as_str().unwrap_or("REGION_CAPACITY").into()),
            message: "the selected plan has no capacity or stock in this region; choose another plan or region".into(),
            details: Some(quote["availability"].clone()),
        }.into());
    }
    Ok(())
}
