//! Command-line interface.

pub mod auth;
pub mod catalog;
pub mod context;
pub mod resources;
pub mod servers;
pub mod update;

use clap::{Parser, Subcommand};

use crate::config::Settings;
use crate::error::Result;
use crate::output::Format;
use context::Ctx;

#[derive(Parser, Debug)]
#[command(
    name = "zy",
    author,
    version,
    about = "Zy — manage your Cloudzy cloud from the terminal or an AI assistant",
    after_help = "Sign in once with `zy login` (or `zy login --device` over SSH). For CI, set CLOUDZY_TOKEN to a developer API token.\nRun `zy <command> --help` for details."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// Cloudzy platform URL [env: CLOUDZY_URL] [default: https://dash.cloudzy.com]
    #[arg(long, global = true, value_name = "URL")]
    pub url: Option<String>,
    /// Named sign-in to use [env: CLOUDZY_PROFILE] [default: default]
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,
    /// Output format
    #[arg(long, short, global = true, value_enum, default_value_t = Format::Table)]
    pub output: Format,
    /// Log API requests and responses to stderr (tokens are never printed)
    #[arg(long, global = true)]
    pub debug: bool,
    /// Disable colorized output
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Sign in to Cloudzy in your browser, or with a device code
    Login(auth::LoginArgs),
    /// Sign out and revoke the stored sign-in
    Logout,
    /// Inspect the current credential
    Auth {
        #[command(subcommand)]
        command: auth::AuthCommand,
    },
    /// Show who you are signed in as
    Whoami,
    /// Servers (virtual machines)
    #[command(visible_alias = "server")]
    Servers {
        #[command(subcommand)]
        command: servers::ServersCommand,
    },
    /// Server snapshots
    #[command(visible_alias = "snapshot")]
    Snapshots {
        #[command(subcommand)]
        command: resources::SnapshotsCommand,
    },
    /// Saved SSH keys
    #[command(name = "ssh-keys", visible_alias = "ssh-key")]
    SshKeys {
        #[command(subcommand)]
        command: resources::SshKeysCommand,
    },
    /// Reserved (floating) IPs
    #[command(name = "reserved-ips", visible_alias = "reserved-ip")]
    ReservedIps {
        #[command(subcommand)]
        command: resources::ReservedIpsCommand,
    },
    /// Public IPs attached to a server
    Ips {
        #[command(subcommand)]
        command: resources::IpsCommand,
    },
    /// Per-server firewall rules
    Firewall {
        #[command(subcommand)]
        command: resources::FirewallCommand,
    },
    /// Regions
    Regions {
        #[command(subcommand)]
        command: catalog::RegionsCommand,
    },
    /// Plans and prices
    Plans {
        #[command(subcommand)]
        command: catalog::PlansCommand,
    },
    /// OS templates
    Os {
        #[command(subcommand)]
        command: catalog::OsCommand,
    },
    /// One-click apps
    Apps {
        #[command(subcommand)]
        command: catalog::AppsCommand,
    },
    /// Balance, ledger and invoices
    Billing {
        #[command(subcommand)]
        command: resources::BillingCommand,
    },
    /// Serve the Model Context Protocol over stdio for AI assistants
    #[command(long_about = "Serve the Model Context Protocol (JSON-RPC over stdin/stdout) so an AI assistant can manage Cloudzy through zy's tools.\n\nUses the same credential as every other command: CLOUDZY_TOKEN, or the sign-in stored by `zy login`.\n\nExample (Claude Code):  claude mcp add cloudzy -- zy mcp")]
    Mcp,
    /// Update zy to the latest release
    Update {
        /// Release channel to check (stable, beta, alpha, rc)
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(cli: Cli) -> Result<()> {
    if !color_wanted(cli.no_color) {
        yansi::whenever(yansi::Condition::NEVER);
    }
    let settings = Settings::resolve(cli.url.as_deref(), cli.profile.as_deref());
    let ctx = Ctx {
        settings,
        format: cli.output,
        debug: cli.debug,
    };
    match &cli.command {
        Command::Login(args) => auth::login(&ctx.settings, args).await,
        Command::Logout => auth::logout(&ctx.settings).await,
        Command::Auth { command } => auth::auth(&ctx.settings, command).await,
        Command::Whoami => resources::whoami(&ctx).await,
        Command::Servers { command } => servers::run(&ctx, command).await,
        Command::Snapshots { command } => resources::snapshots(&ctx, command).await,
        Command::SshKeys { command } => resources::ssh_keys(&ctx, command).await,
        Command::ReservedIps { command } => resources::reserved_ips(&ctx, command).await,
        Command::Ips { command } => resources::ips(&ctx, command).await,
        Command::Firewall { command } => resources::firewall(&ctx, command).await,
        Command::Regions { command } => catalog::regions(&ctx, command).await,
        Command::Plans { command } => catalog::plans(&ctx, command).await,
        Command::Os { command } => catalog::os(&ctx, command).await,
        Command::Apps { command } => catalog::apps(&ctx, command).await,
        Command::Billing { command } => resources::billing(&ctx, command).await,
        Command::Mcp => {
            let backend = ctx.api().map_err(|e| e.to_string());
            if let Err(why) = &backend {
                eprintln!("zy mcp: {why}");
            }
            crate::mcp::server::run(crate::mcp::server::Server::new(backend))
                .await
                .map_err(|e| crate::error::CliError::Other(format!("mcp stdio: {e}")))
        }
        Command::Update { channel, force } => {
            update::run(channel, *force).await;
            Ok(())
        }
    }
}

/// Colour only for a person at a terminal: not with `--no-color`, not when
/// NO_COLOR is set (no-color.org), and not when stderr — where zy writes
/// its coloured notes — is redirected to a file or a pipe.
fn color_wanted(flag: bool) -> bool {
    use std::io::IsTerminal;
    !flag
        && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
        && std::io::stderr().is_terminal()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
