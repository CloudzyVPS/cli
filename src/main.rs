use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use yansi::Paint;

#[tokio::main]
async fn main() {
    let cli = zy::commands::Cli::parse();
    // Diagnostics go to stderr and stay quiet unless asked for with
    // RUST_LOG=debug; stdout is reserved for command output and MCP.
    let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    let filter = if cli.debug {
        filter.add_directive("zy=debug".parse().unwrap())
    } else {
        filter
    };
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(filter)
        .init();

    let json_output = cli.output == zy::output::Format::Json;
    if let Err(err) = zy::commands::run(cli).await {
        if json_output {
            let value = match &err {
                zy::error::CliError::Api(error) => error.to_json(),
                other => serde_json::json!({"error": other.to_string()}),
            };
            zy::output::print_json(&value);
        }
        eprintln!("{} {err}", "error:".red().bold());
        std::process::exit(err.exit_code());
    }
}
