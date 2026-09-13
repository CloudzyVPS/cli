use clap::{Parser, Subcommand};
use std::process;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use zy::update;

#[derive(Parser)]
#[command(name = "zy", author, version, about = "Zy — the Cloudzy command-line tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Disable colorized output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Update the Zy CLI to the latest version
    Update {
        /// Release channel to check (stable, beta, alpha, rc)
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Skip confirmation prompt and update immediately
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    if cli.no_color {
        yansi::whenever(yansi::Condition::NEVER);
    }

    match cli.command {
        Commands::Update { channel, force } => {
            let channel = match channel.to_lowercase().as_str() {
                "beta" => update::Channel::Beta,
                "alpha" => update::Channel::Alpha,
                "rc" => update::Channel::ReleaseCandidate,
                _ => update::Channel::Stable,
            };

            match update::check_for_update(channel).await {
                Ok(Some(release)) => {
                    // Show release information
                    println!("\n{}", yansi::Paint::new("New version available!").green().bold());
                    println!("  Current version:  {}", yansi::Paint::new(update::Version::current().to_string()).cyan());
                    println!("  Latest version:   {}", yansi::Paint::new(release.version.to_string()).cyan().bold());
                    println!("  Release page:     {}", yansi::Paint::new(&release.download_url).underline());
                    
                    // Calculate download size
                    let platform = update::Platform::current();
                    if let Ok(asset) = update::select_asset_for_platform(&release.assets, &platform) {
                        let size_mb = asset.size as f64 / (1024.0 * 1024.0);
                        println!("  Download size:    {:.2} MB", size_mb);
                    }
                    
                    // Prompt for confirmation unless --force is used
                    if !force {
                        println!("\n{}", yansi::Paint::new("Do you want to download and install this update? [y/N]").yellow());
                        
                        let mut input = String::new();
                        if let Err(e) = std::io::stdin().read_line(&mut input) {
                            eprintln!("{}: {}", yansi::Paint::new("Failed to read input").red(), e);
                            process::exit(1);
                        }
                        
                        let input = input.trim().to_lowercase();
                        if input != "y" && input != "yes" {
                            println!("{}", yansi::Paint::new("Update cancelled.").yellow());
                            return;
                        }
                    }
                    
                    // Perform the update
                    match update::perform_update(release).await {
                        Ok(_) => {
                            println!("\n{}", yansi::Paint::new("Update completed successfully!").green().bold());
                            println!("{}", yansi::Paint::new("Please restart the CLI to use the new version.").yellow());
                        }
                        Err(e) => {
                            eprintln!("\n{}: {}", yansi::Paint::new("Update failed").red().bold(), e);
                            eprintln!("{}", yansi::Paint::new("Your original binary has been restored.").yellow());
                            process::exit(1);
                        }
                    }
                }
                Ok(None) => {
                    // Already on latest version - message already printed by check_for_update
                }
                Err(e) => {
                    eprintln!("{}: {}", yansi::Paint::new("Error checking for updates").red(), e);
                    process::exit(1);
                }
            }
            return;
        }
    }
}
