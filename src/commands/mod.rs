//! Command-line interface.

pub mod auth;
pub mod update;

use clap::{Parser, Subcommand};

use crate::config::Settings;
use crate::error::Result;

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
    if cli.no_color {
        yansi::whenever(yansi::Condition::NEVER);
    }
    let settings = Settings::resolve(cli.url.as_deref(), cli.profile.as_deref());
    match &cli.command {
        Command::Login(args) => auth::login(&settings, args).await,
        Command::Logout => auth::logout(&settings).await,
        Command::Auth { command } => auth::auth(&settings, command).await,
        Command::Update { channel, force } => {
            update::run(channel, *force).await;
            Ok(())
        }
    }
}
