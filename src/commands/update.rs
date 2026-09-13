//! `zy update`.

use std::process;

use crate::update;

pub async fn run(channel: &str, force: bool) {
    let channel = match channel.to_lowercase().as_str() {
        "beta" => update::Channel::Beta,
        "alpha" => update::Channel::Alpha,
        "rc" => update::Channel::ReleaseCandidate,
        _ => update::Channel::Stable,
    };

    match update::check_for_update(channel).await {
        Ok(Some(release)) => {
            // Show release information
            println!(
                "\n{}",
                yansi::Paint::new("New version available!").green().bold()
            );
            println!(
                "  Current version:  {}",
                yansi::Paint::new(update::Version::current().to_string()).cyan()
            );
            println!(
                "  Latest version:   {}",
                yansi::Paint::new(release.version.to_string()).cyan().bold()
            );
            println!(
                "  Release page:     {}",
                yansi::Paint::new(&release.download_url).underline()
            );

            // Calculate download size
            let platform = update::Platform::current();
            if let Ok(asset) = update::select_asset_for_platform(&release.assets, &platform) {
                let size_mb = asset.size as f64 / (1024.0 * 1024.0);
                println!("  Download size:    {:.2} MB", size_mb);
            }

            // Prompt for confirmation unless --force is used
            if !force {
                println!(
                    "\n{}",
                    yansi::Paint::new("Do you want to download and install this update? [y/N]")
                        .yellow()
                );

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
                    println!(
                        "\n{}",
                        yansi::Paint::new("Update completed successfully!")
                            .green()
                            .bold()
                    );
                    println!(
                        "{}",
                        yansi::Paint::new("Please restart the CLI to use the new version.")
                            .yellow()
                    );
                }
                Err(e) => {
                    eprintln!(
                        "\n{}: {}",
                        yansi::Paint::new("Update failed").red().bold(),
                        e
                    );
                    eprintln!(
                        "{}",
                        yansi::Paint::new("Your original binary has been restored.").yellow()
                    );
                    process::exit(1);
                }
            }
        }
        Ok(None) => {
            // Already on latest version - message already printed by check_for_update
        }
        Err(e) => {
            eprintln!(
                "{}: {}",
                yansi::Paint::new("Error checking for updates").red(),
                e
            );
            process::exit(1);
        }
    }
}
