use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use yansi::Paint;

#[tokio::main]
async fn main() {
    // Diagnostics go to stderr and stay quiet unless asked for with
    // RUST_LOG=debug; stdout is reserved for command output and MCP.
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let cli = zy::commands::Cli::parse();
    if let Err(err) = zy::commands::run(cli).await {
        eprintln!("{} {err}", "error:".red().bold());
        std::process::exit(err.exit_code());
    }
}
