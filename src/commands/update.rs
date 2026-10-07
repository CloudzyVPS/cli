//! `zy update`.

use super::context::Ctx;
use crate::error::{CliError, Result};
use crate::output::print_json;
use crate::update::{self, Channel, GitHubClient, Version};
use serde_json::json;

pub async fn run(ctx: &Ctx, channel: &str, force: bool) -> Result<()> {
    let channel = match channel {
        "stable" => Channel::Stable,
        "beta" => Channel::Beta,
        "alpha" => Channel::Alpha,
        "rc" => Channel::ReleaseCandidate,
        _ => {
            return Err(CliError::Usage(
                "channel must be stable, beta, alpha or rc".into(),
            ))
        }
    };
    let current = Version::current();
    let release = GitHubClient::new(update::REPO_OWNER.into(), update::REPO_NAME.into())
        .get_latest_release(channel)
        .await
        .map_err(|e| CliError::Other(e.to_string()))?;
    let available = release.version.is_newer_than(&current);
    // JSON checks never install or prompt; --force explicitly requests installation.
    if ctx.json() && !force {
        print_json(
            &json!({ "currentVersion": current.to_string(), "latestVersion": release.version.to_string(),
            "updateAvailable": available, "releaseUrl": release.download_url }),
        );
        return Ok(());
    }
    eprintln!("Current: {current}; latest: {}.", release.version);
    if !available {
        if ctx.json() {
            print_json(&json!({"updated": false, "version": current.to_string()}));
        } else {
            eprintln!("You are already running the latest version.");
        }
        return Ok(());
    }
    if !force {
        use std::io::{IsTerminal, Write};
        if !std::io::stdin().is_terminal() {
            return Err(CliError::Usage(
                "pass --force to install an update non-interactively".into(),
            ));
        }
        eprint!("Install {}? [y/N] ", release.version);
        std::io::stderr()
            .flush()
            .map_err(|e| CliError::Other(e.to_string()))?;
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| CliError::Other(e.to_string()))?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            eprintln!("Update cancelled.");
            return Ok(());
        }
    }
    let version = release.version.to_string();
    update::perform_update(release)
        .await
        .map_err(|e| CliError::Other(format!("update failed: {e}")))?;
    if ctx.json() {
        print_json(&json!({"updated": true, "version": version}));
    } else {
        ctx.done(format!("Updated to {version}. Restart zy to use it."));
    }
    Ok(())
}
