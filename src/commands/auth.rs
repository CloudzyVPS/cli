//! `zy login`, `zy logout`, `zy auth …`.

use std::io::IsTerminal;

use clap::{Args, Subcommand};
use yansi::Paint;

use crate::auth::session::Credential;
use crate::auth::store::FileStore;
use crate::auth::{browser, device, discovery, now_secs, oauth, scope_string, AuthError};
use crate::config::Settings;
use crate::error::Result;

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// Sign in with a short code approved on another device, for hosts
    /// without a browser (SSH sessions, containers)
    #[arg(long)]
    pub device: bool,
    /// Print the sign-in URL instead of opening a browser
    #[arg(long, conflicts_with = "device")]
    pub no_browser: bool,
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Show which credential zy would use
    Status,
    /// Print a valid access token for use with other tools
    Token,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("zy/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client")
}

/// Whether a browser can plausibly be opened from this session.
fn browser_available() -> bool {
    if std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some() {
        return false;
    }
    if cfg!(target_os = "linux") {
        return std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
    }
    true
}

pub async fn login(settings: &Settings, args: &LoginArgs) -> Result<()> {
    let http = http_client();
    let meta = discovery::discover(&http, &settings.url).await?;
    let scope = scope_string();
    let use_device = args.device || (!args.no_browser && !browser_available());

    let tokens = if use_device {
        let endpoint = meta.device_authorization_endpoint.clone().ok_or_else(|| {
            AuthError::Protocol(format!(
                "{} does not offer device sign-in; run `zy login` on a machine with a browser",
                settings.url
            ))
        })?;
        let start = device::start(&http, &endpoint, &scope).await?;
        let link = start
            .verification_uri_complete
            .clone()
            .unwrap_or_else(|| start.verification_uri.clone());
        eprintln!();
        eprintln!("  To sign in, open {}", link.as_str().cyan().underline());
        eprintln!(
            "  and confirm this code:  {}",
            start.user_code.as_str().bold().yellow()
        );
        eprintln!();
        eprintln!("  Only approve if you started this sign-in yourself. Waiting…");
        device::Poller::default()
            .poll(&http, &meta.token_endpoint, &start)
            .await?
    } else {
        let no_browser = args.no_browser;
        browser::login(
            &http,
            &meta,
            &scope,
            |url| !no_browser && open::that_detached(url).is_ok(),
            |url, opened| {
                eprintln!();
                if opened {
                    eprintln!("  Opened your browser to sign in to Cloudzy. If nothing happened, open:");
                } else {
                    eprintln!("  Open this URL in your browser to sign in to Cloudzy:");
                }
                eprintln!("  {}", url.cyan().underline());
                eprintln!();
                eprintln!("  Waiting for the browser… (Ctrl+C to cancel; `zy login --device` works without one)");
            },
        )
        .await?
    };

    let profile = tokens.into_profile(&settings.url)?;
    FileStore::new(settings.config_dir.clone()).save(&settings.profile, &profile)?;
    eprintln!(
        "{} Signed in to {} (profile {}).",
        "✓".green(),
        settings.url.as_str().bold(),
        settings.profile.as_str().bold()
    );
    if settings.env_token.is_some() {
        eprintln!(
            "{} CLOUDZY_TOKEN is set and takes precedence over this sign-in until you unset it.",
            "!".yellow()
        );
    }
    Ok(())
}

pub async fn logout(settings: &Settings) -> Result<()> {
    let store = FileStore::new(settings.config_dir.clone());
    let Some(profile) = store.load(&settings.profile)? else {
        eprintln!("Not signed in (profile {}).", settings.profile);
        return Ok(());
    };
    let http = http_client();
    // Revoke server-side first so a copied credentials file stops working
    // too; forget locally even when the platform cannot be reached.
    let revoked = match discovery::discover(&http, &profile.url).await {
        Ok(meta) => match meta.revocation_endpoint {
            Some(ep) => oauth::revoke(&http, &ep, &profile.refresh_token)
                .await
                .is_ok(),
            None => false,
        },
        Err(_) => false,
    };
    store.remove(&settings.profile)?;
    if revoked {
        eprintln!(
            "{} Signed out of {} and revoked the session.",
            "✓".green(),
            profile.url
        );
    } else {
        eprintln!(
            "{} Removed the local sign-in, but could not revoke it on {}. Withdraw \"Cloudzy CLI\" under Settings → Security → Connected applications to be sure.",
            "!".yellow(),
            profile.url
        );
    }
    Ok(())
}

pub async fn auth(settings: &Settings, cmd: &AuthCommand) -> Result<()> {
    let credential = Credential::resolve(settings, http_client())?;
    match cmd {
        AuthCommand::Token => {
            let token = credential.bearer().await?;
            if std::io::stdout().is_terminal() {
                eprintln!("{} This is a bearer credential; do not paste it anywhere you would not paste a password.", "!".yellow());
            }
            println!("{token}");
        }
        AuthCommand::Status => {
            println!("URL:        {}", settings.url);
            println!("Profile:    {}", settings.profile);
            println!("Credential: {}", credential.describe());
            if let Credential::Session(session) = &credential {
                let snap = session.snapshot().await;
                let left = snap.expires_at.saturating_sub(now_secs());
                println!(
                    "Access:     {}",
                    if left > 0 {
                        format!("valid for {}m (refreshed automatically)", left / 60)
                    } else {
                        "expired — will refresh on next use".into()
                    }
                );
                println!("Scopes:     {}", snap.scope);
            }
        }
    }
    Ok(())
}
