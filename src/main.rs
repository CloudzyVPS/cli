mod models;
mod services;
mod utils;
mod api;
mod templates;
mod handlers;
mod update;

use zy::config;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderValue;
use tower::ServiceBuilder;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::process;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use comfy_table::{Table, presets, modifiers, ContentArrangement};
use terminal_size::{Width, terminal_size};

use config::{DEFAULT_HOST, DEFAULT_PORT};
use models::{UserRecord, AppState};
use services::{generate_password_hash, load_users_from_file, persist_users_file, simple_instance_action};
use handlers::helpers::api_call_wrapper;

// Embed the default stylesheet in the binary
const DEFAULT_STYLESHEET: &str = include_str!("../static/styles.css");

async fn build_state_from_env(env_file: Option<&str>) -> AppState {
    config::load_env_file(env_file);
    let users = load_users_from_file().await;
    let disabled_instances = std::sync::Arc::new(config::get_disabled_instance_ids());
    
    let current_hostname = std::process::Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let client = reqwest::Client::builder()
        .user_agent(format!("Zy/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to create HTTP client");
    
    AppState {
        users,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        flash_store: Arc::new(Mutex::new(HashMap::new())),
        default_customer_cache: Arc::new(Mutex::new(None)),
        api_base_url: config::get_api_base_url(),
        api_token: config::get_api_token(),
        public_base_url: config::get_public_base_url(),
        client,
        disabled_instances,
        current_hostname,
        custom_css: None,
    }
}

// Global template context injected into most page templates
// (already implemented via build_template_globals/TemplateGlobals)
fn build_app(state: AppState) -> Router {
    let protected_routes = Router::new()
        .route("/users", get(handlers::users::users_list).post(handlers::users::users_create))
        .route("/users/:username", get(handlers::users::user_detail))
        .route("/users/:username/reset-password", post(handlers::users::reset_password))
        .route("/users/:username/role", post(handlers::users::update_role))
        .route("/users/:username/about", post(handlers::users::update_about))
        .route("/users/:username/delete", post(handlers::users::delete_user))
        .route("/access", get(handlers::access::access_get))
        .route("/access/:username", post(handlers::access::update_access))
        .route("/ssh-keys", get(handlers::ssh_keys::ssh_keys_get).post(handlers::ssh_keys::ssh_keys_post))
        .route("/instances", get(handlers::instances::instances_real))
        .route("/regions", get(handlers::catalog::regions_get))
        .route("/products", get(handlers::catalog::products_get))
        .route("/os", get(handlers::catalog::os_get))
        .route("/applications", get(handlers::catalog::applications_get))
        .route("/create/step-1", get(handlers::wizard::create_step_1))
        .route("/create/step-2", get(handlers::wizard::create_step_2))
        .route("/create/step-3", get(handlers::wizard::create_step_3))
        .route("/create/step-4", get(handlers::wizard::create_step_4))
        .route("/create/step-5", get(handlers::wizard::create_step_5))
        .route("/create/step-6", get(handlers::wizard::create_step_6))
        .route(
            "/create/step-7",
            get(handlers::wizard::create_step_7_get).post(handlers::wizard::create_step_7_post),
        )
        .route("/create/result", get(handlers::wizard::create_step_8))
        .route("/instance/:instance_id", get(handlers::instances::instance_detail))
        .route("/instance/:instance_id/delete", post(handlers::instances::instance_delete))
        .route("/instance/:instance_id/poweron", post(handlers::instances::instance_poweron_post))
        .route("/instance/:instance_id/poweroff", post(handlers::instances::instance_poweroff_post))
        .route("/instance/:instance_id/reset", post(handlers::instances::instance_reset_post))
        .route("/about", get(handlers::system::about_get))
        .route("/about/check-update", post(handlers::system::about_check_update))
        .route("/about/switch-version", post(handlers::system::about_switch_version))
        .route("/confirm/:action/:id", get(handlers::system::confirmation_get))
        .route(
            "/instance/:instance_id/change-pass",
            get(handlers::instances::instance_change_pass_get).post(handlers::instances::instance_change_pass_post),
        )
        .route("/instance/:instance_id/change-os", get(handlers::instances::instance_change_os_get).post(handlers::instances::instance_change_os_post))
        .route("/instance/:instance_id/resize", get(handlers::instances::instance_resize_get).post(handlers::instances::instance_resize_post))
        .route(
            "/instance/:instance_id/add-traffic",
            post(handlers::instances::instance_add_traffic),
        )
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), handlers::middleware::auth_middleware));

    // Always serve styles.css - use custom if provided, otherwise use embedded default
    let stylesheet_content = state.custom_css.clone().unwrap_or_else(|| DEFAULT_STYLESHEET.to_string());

    let app = Router::new()
        .route("/", get(handlers::auth::root_get))
        .route("/login", get(handlers::auth::login_get).post(handlers::auth::login_post))
        .route("/logout", post(handlers::auth::logout_post))
        .route("/static/styles.css", get(move || {
            let css = stylesheet_content.clone();
            async move {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css")],
                    css
                )
            }
        }))
        .merge(protected_routes);

    app.nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new("static")),
        )
        .with_state(state)
}

async fn start_server(mut state: AppState, host: &str, port: u16, stylesheet: Option<String>) {
    if let Some(path) = stylesheet {
        match std::fs::read_to_string(&path) {
            Ok(css) => {
                state.custom_css = Some(css);
                tracing::info!("Loaded custom stylesheet from {}", path);
            }
            Err(e) => {
                tracing::error!(%e, "Failed to read custom stylesheet");
                eprintln!("{} {}: {}", yansi::Paint::red("Failed to read custom stylesheet at"), path, e);
                process::exit(1);
            }
        }
    }

    let addr: SocketAddr = match format!("{}:{}", host, port).parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(%e, "Invalid host/port format");
            eprintln!("{}: {}", yansi::Paint::red("Invalid host/port format"), e);
            process::exit(1);
        }
    };
    let app = build_app(state.clone());
    tracing::info!(%addr, "Starting Zy Rust server");
    println!("{} {}", yansi::Paint::new("Web server running on").green(), yansi::Paint::new(format!("http://{}", addr)).cyan());
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            // Run the server and log any errors (do not panic with unwrap()).
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!(%e, "Server encountered an error while running");
                eprintln!("{}: {}", yansi::Paint::new("Server error").red(), e);
                process::exit(1);
            }
        }
        Err(e) => {
            tracing::error!(%e, "Failed to bind to address; is the port already in use?");
            eprintln!("{}: {}\n{}", yansi::Paint::new(format!("Failed to bind to {}", addr)).red(), e, yansi::Paint::new("Please stop any process using this port, or start the server with a different --port value.").yellow());
            process::exit(1);
        }
    }
}


fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
             serde_json::to_string(v).unwrap_or_default()
        }
    }
}

fn print_table(value: &serde_json::Value) {
    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL);
    table.apply_modifier(modifiers::UTF8_ROUND_CORNERS);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    
    if let Some((Width(w), _)) = terminal_size() {
        table.set_width(w - 4);
    }

    match value {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                println!("(empty list)");
                return;
            }
            // Try to find a non-empty object to get keys from, or union of keys?
            // For simplicity, use the first object's keys if available.
            if let Some(first) = arr.iter().find_map(|v| v.as_object()) {
                let headers: Vec<&String> = first.keys().collect();
                table.set_header(&headers);
                
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        let row: Vec<String> = headers.iter().map(|k| {
                            obj.get(*k).map(json_value_to_string).unwrap_or_default()
                        }).collect();
                        table.add_row(row);
                    }
                }
            } else {
                // List of primitives
                table.set_header(vec!["Value"]);
                for item in arr {
                    table.add_row(vec![json_value_to_string(item)]);
                }
            }
        },
        serde_json::Value::Object(obj) => {
            table.set_header(vec!["Field", "Value"]);
            for (k, v) in obj {
                table.add_row(vec![k, &json_value_to_string(v)]);
            }
        },
        _ => {
            println!("{}", json_value_to_string(value));
            return;
        }
    }
    
    println!("\n{table}\n");
}

fn print_api_response(value: &serde_json::Value) {
    if let Some(obj) = value.as_object() {
        // Check for standard envelope
        if obj.contains_key("code") && obj.contains_key("data") {
             if let Some(detail) = obj.get("detail").and_then(|v| v.as_str()) {
                 println!("{}", detail);
             }
             let data = &obj["data"];
             print_table(data);
             return;
        }
    }
    print_table(value);
}



#[derive(Parser)]
#[command(
    name = "zy",
     author,
     version,
    about = "Zy command-line tool",
    long_about = r#"Zy — control and manage your Cloudzy services right from home.

This tool surfaces a small set of commands to run the server, validate configuration, manage local users and manage instances through the API. Use the `--env-file` option or environment variables to provide API credentials.

Examples:
  1) Build & run (dev):
      cargo run -- serve --host 127.0.0.1 --port 5000
  2) Build a release binary:
      cargo build --release
    3) Manage instances:
          zy instances list
          zy instances show 12345
"#, 
    after_help = "Use `zy <subcommand> --help` to get subcommand specific options and usage examples."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Disable colorized output
    #[arg(long, global = true)]
    no_color: bool,
    /// Disable request/response logging
    #[arg(long, global = true)]
    silent: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web server
    Serve {
        /// Host to bind to
        #[arg(long, default_value_t = String::from(DEFAULT_HOST))]
        host: String,
        /// Port to bind to
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Path to .env file
        #[arg(long)]
        env_file: Option<String>,
        /// Path to a custom stylesheet to serve instead of the default
        #[arg(long)]
        stylesheet: Option<String>,
    },
    /// Validate configuration (env vars / API credentials)
    #[command(about = "Validate configuration and ensure API connectivity.", long_about = "Validate environment variables required for the Zy server, and optionally validate the configured API token by attempting to fetch regions from the remote API.")]
    CheckConfig { env_file: Option<String> },
    /// Manage local users (users.json)
    Users {
        #[command(subcommand)]
        sub: UserCommands,
    },
    /// Manage instances via the configured API
    #[command(about = "Manage compute instances via the API (list, show, power, delete, etc.)", long_about = "These commands perform the same actions that the web UI's instance actions perform; they make API requests using the current API configuration and token. Be careful with commands that mutate state (delete, reset). Use `--help` on a subcommand for detailed examples.")]
    Instances {
        #[command(subcommand)]
        sub: InstanceCommands,
    },
    /// Update the Zy CLI to the latest version
    Update {
        /// Release channel to check (stable, beta, alpha, rc)
        #[arg(long, default_value = "stable")]
        channel: String,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    #[command(about = "List current users", long_about = "Enumerate users stored in users.json (username, role, assigned_instances).")]
    List,
    #[command(about = "Add a new user", long_about = "Add a user with a role (owner|admin). The password will be hashed and saved to users.json.")]
    Add {
        username: String,
        password: String,
        role: String,
    },
    /// Add a new owner user (use --force to overwrite existing owner user(s))
    #[command(about = "Add an owner user", long_about = "Create a new owner user. Use --force to overwrite an existing owner user or create another owner.")]
    AddOwner {
        username: String,
        password: String,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    #[command(about = "Reset a user's password", long_about = "Set a new password for an existing user; password will be hashed.")]
    ResetPassword {
        username: String,
        password: String,
    },
}

#[derive(Subcommand)]
enum InstanceCommands {
    /// List instances (optional --username to filter)
    #[command(about = "List instances", long_about = "List instances the configured API user may access. Provide `--username` to filter instances assigned to a local user. Use `--page` and `--per-page` for pagination.")]
    List {
        /// Optional username to filter instances by assigned user (use empty to list all)
        #[arg(long)]
        username: Option<String>,
        /// Page number to display (1-indexed). Use 0 to show all instances without pagination.
        #[arg(long, short = 'p', default_value = "0")]
        page: usize,
        /// Number of instances per page (default: 20, only used when page > 0)
        #[arg(long, default_value = "20")]
        per_page: usize,
    },
    /// Show instance details
    #[command(about = "Show instance details", long_about = "Show the raw JSON payload returned by the API for an instance ID.")]
    Show { instance_id: String },
    /// Power on an instance
    #[command(about = "Power on an instance", long_about = "Request an asynchronous power-on operation for an instance; the API may perform the action asynchronously.")]
    PowerOn { instance_id: String },
    /// Power off an instance
    #[command(about = "Power off an instance", long_about = "Request an asynchronous power-off operation for an instance; follow up with `show` to confirm state.")]
    PowerOff { instance_id: String },
    /// Reset an instance
    #[command(about = "Reset an instance", long_about = "Request an immediate reset/reboot of the instance. This is destructive to running state but usually preserves disks.")]
    Reset { instance_id: String },
    /// Delete an instance
    #[command(about = "Delete an instance", long_about = "Permanently delete an instance. Use with care and confirm `id` and `username` if necessary.")]
    Delete { instance_id: String },
    /// Change the instance password (prints the generated password)
    #[command(about = "Change root/console password", long_about = "Generate and set a new root/console password for an instance and print the generated value (if API returns it).")]
    ChangePass { instance_id: String },
    /// Change the instance OS
    #[command(about = "Change the instance OS", long_about = "Trigger an OS distribution and image change. Provide a valid `os_id` from the remote API.")]
    ChangeOs { instance_id: String, os_id: String },
    /// Resize the instance (type: FIXED|CUSTOM — for CUSTOM specify cpu,ram,disk etc.)
    #[command(about = "Resize an instance", long_about = "Change a plan; specify `--type FIXED` with `--product-id` or `--type CUSTOM` with specific resource values (cpu, ram-in-gb, disk-in-gb, bandwidth-in-tb).")]
    Resize { instance_id: String, #[arg(long)] r#type: String, #[arg(long)] product_id: Option<String>, #[arg(long)] cpu: Option<i64>, #[arg(long)] ram_in_gb: Option<i64>, #[arg(long)] disk_in_gb: Option<i64>, #[arg(long)] bandwidth_in_tb: Option<i64> },
    /// Add traffic amount (e.g., 50) to an instance
    #[command(about = "Add traffic to an instance", long_about = "Add additional traffic capacity to an instance using a numeric `--amount` (e.g., 50).")]
    AddTraffic { instance_id: String, amount: f64 },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // CLI parsing
    let cli = Cli::parse();

    if cli.no_color {
        yansi::whenever(yansi::Condition::NEVER);
    }

    if cli.silent {
        crate::api::client::set_silent(true);
    }

    // If CLI provided an env-file or not, we will load it per command below
    // Note: we avoid constructing a default `state` here; commands build the per-command state
    // using `build_state_from_env` so we can pass a custom `--env-file` when executing commands.

    // Dispatch CLI commands. If no command provided, serve the web app by default
    if cli.command.is_none() {
        let state = build_state_from_env(None).await;
        start_server(state, DEFAULT_HOST, DEFAULT_PORT, None).await;
        return;
    }
    match cli.command.unwrap() {
        Commands::Serve {
            host,
            port,
            env_file,
            stylesheet,
        } => {
            let state = build_state_from_env(env_file.as_deref()).await;
            start_server(state, &host, port, stylesheet).await;
            return;
        }
        Commands::CheckConfig { env_file } => {
            let state = build_state_from_env(env_file.as_deref()).await;
            // Basic check: ensure API base and token exist; optionally ping regions
            let mut ok = true;
            if state.api_base_url.trim().is_empty() {
                eprintln!("{}", yansi::Paint::new("API_BASE_URL is not configured").red());
                ok = false;
            }
            if state.api_token.trim().is_empty() {
                eprintln!("{}", yansi::Paint::new("API_TOKEN is not configured").red());
                ok = false;
            }
            if !ok {
                process::exit(1);
            }
            let resp = api_call_wrapper(&state, "GET", "/v1/regions", None, None).await;
            if resp.get("code").and_then(|c| c.as_str()) == Some("OKAY") {
                println!("{}", yansi::Paint::new("Configuration looks valid (regions returned)").green());
                process::exit(0);
            } else {
                let json_str = serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "<non-json>".into());
                eprintln!(
                    "{}: {}",
                    yansi::Paint::new("Configuration appears invalid").red(),
                    json_str
                );
                process::exit(1);
            }
        }
        Commands::Users { sub } => {
            let state = build_state_from_env(None).await;
            match sub {
                UserCommands::List => {
                    let users = state.users.lock().unwrap();
                    println!("{}", yansi::Paint::new("username\trole\tassigned_instances").bold().underline());
                    for (u, rec) in users.iter() {
                        let assigned = if rec.assigned_instances.is_empty() {
                            String::new()
                        } else {
                            rec.assigned_instances.join(", ")
                        };
                        println!("{}\t{}\t{}", u, rec.role, assigned);
                    }
                    return;
                }
                UserCommands::Add {
                    username,
                    password,
                    role,
                } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    if users.contains_key(&uname) {
                        eprintln!("{} '{}' {}", yansi::Paint::new("User").red(), uname, yansi::Paint::new("already exists").red());
                        process::exit(1);
                    }
                    let hash = generate_password_hash(&password);
                    users.insert(
                        uname.clone(),
                        UserRecord {
                            password: hash,
                            role: role.clone(),
                            assigned_instances: vec![],
                            about: String::new(),
                        },
                    );
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("{}: {}", yansi::Paint::new("Failed to persist users.json").red(), e);
                        process::exit(1);
                    }
                    println!("{} '{}' {}", yansi::Paint::new("User").green(), uname, yansi::Paint::new("added").green());
                    return;
                }
                UserCommands::ResetPassword { username, password } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    if let Some(rec) = users.get_mut(&uname) {
                        rec.password = generate_password_hash(&password);
                    } else {
                        eprintln!("{} '{}' {}", yansi::Paint::new("User").red(), uname, yansi::Paint::new("not found").red());
                        process::exit(1);
                    }
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("{}: {}", yansi::Paint::new("Failed to persist users.json").red(), e);
                        process::exit(1);
                    }
                    println!("{} '{}' {}", yansi::Paint::new("Password for").green(), uname, yansi::Paint::new("updated").green());
                    return;
                }
                UserCommands::AddOwner {
                    username,
                    password,
                    force,
                } => {
                    let uname = username.trim().to_lowercase();
                    let mut users = state.users.lock().unwrap();
                    // If an owner already exists and we're not forcing, error out
                    let owner_exists = users.values().any(|r| r.role == "owner");
                    if owner_exists && !force {
                        eprintln!(
                            "{}", yansi::Paint::new("An owner user already exists; use --force to create another owner or overwrite").red()
                        );
                        process::exit(1);
                    }
                    // If the username exists and force is not set, fail (consistent with `Add` semantics)
                    if users.contains_key(&uname) && !force {
                        eprintln!("{} '{}' {}; {}", yansi::Paint::new("User").red(), uname, yansi::Paint::new("already exists").red(), yansi::Paint::new("use --force to overwrite").yellow());
                        process::exit(1);
                    }
                    let hash = generate_password_hash(&password);
                    users.insert(
                        uname.clone(),
                        UserRecord {
                            password: hash,
                            role: "owner".to_string(),
                            assigned_instances: vec![],
                            about: String::new(),
                        },
                    );
                    drop(users);
                    if let Err(e) = persist_users_file(&state.users).await {
                        eprintln!("{}: {}", yansi::Paint::new("Failed to persist users.json").red(), e);
                        process::exit(1);
                    }
                    println!("{} '{}' {}", yansi::Paint::new("Owner").green(), uname, yansi::Paint::new("created").green());
                    return;
                }
            }
        }
        Commands::Instances { sub } => {
            let state = build_state_from_env(None).await;
            match sub {
                InstanceCommands::List { username, page, per_page } => {
                    let uname = username.unwrap_or_default();
                    let paginated = handlers::helpers::load_instances_for_user_paginated(&state, &uname, page, per_page).await;
                    
                    let mut table = Table::new();
                    table.load_preset(presets::UTF8_FULL);
                    table.apply_modifier(modifiers::UTF8_ROUND_CORNERS);
                    table.set_content_arrangement(ContentArrangement::Dynamic);
                    if let Some((Width(w), _)) = terminal_size() {
                        table.set_width(w - 4);
                    }
                    table.set_header(vec!["ID", "Hostname", "Status"]);
                    for i in &paginated.instances {
                        table.add_row(vec![&i.id, &i.hostname, &i.status]);
                    }
                    println!("\n{table}");
                    
                    // Display pagination information
                    if page > 0 && paginated.total_pages > 1 {
                        println!("\n{}", yansi::Paint::new(format!(
                            "Page {} of {} | Showing {} of {} total instances",
                            paginated.current_page,
                            paginated.total_pages,
                            paginated.instances.len(),
                            paginated.total_count
                        )).cyan());
                        
                        if paginated.current_page > 1 {
                            println!(
                                "{} {}",
                                yansi::Paint::new("←").bold(),
                                yansi::Paint::new(format!("Previous page: zy instances list --page {} --per-page {}", paginated.current_page - 1, per_page)).dim()
                            );
                        }
                        if paginated.current_page < paginated.total_pages {
                            println!(
                                "{} {}",
                                yansi::Paint::new("→").bold(),
                                yansi::Paint::new(format!("Next page: zy instances list --page {} --per-page {}", paginated.current_page + 1, per_page)).dim()
                            );
                        }
                    } else if page == 0 {
                        println!("\n{}", yansi::Paint::new(format!(
                            "Showing all {} instances (use --page 1 --per-page 20 to enable pagination)",
                            paginated.total_count
                        )).dim());
                    }
                    println!();
                    return;
                }
                InstanceCommands::Show { instance_id } => {
                    let endpoint = format!("/v1/instances/{}", instance_id);
                    let payload = api_call_wrapper(&state, "GET", &endpoint, None, None).await;
                    print_api_response(&payload);
                    return;
                }
                InstanceCommands::PowerOn { instance_id } => {
                    let payload = simple_instance_action(&state, "poweron", &instance_id).await;
                    print_api_response(&payload);
                    return;
                }
                InstanceCommands::PowerOff { instance_id } => {
                    let payload = simple_instance_action(&state, "poweroff", &instance_id).await;
                    print_api_response(&payload);
                    return;
                }
                InstanceCommands::Reset { instance_id } => {
                    let payload = simple_instance_action(&state, "reset", &instance_id).await;
                    print_api_response(&payload);
                    return;
                }
                InstanceCommands::Delete { instance_id } => {
                    let endpoint = format!("/v1/instances/{}", instance_id);
                    let payload = api_call_wrapper(&state, "DELETE", &endpoint, None, None).await;
                    print_api_response(&payload);
                    return;
                }
                InstanceCommands::ChangePass { instance_id } => {
                    let endpoint = format!("/v1/instances/{}/change-pass", instance_id);
                    let payload = api_call_wrapper(&state, "POST", &endpoint, None, None).await;
                    if let Some(pass) = payload.get("data").and_then(|d| d.get("password")).and_then(|v| v.as_str()) {
                        println!("{} {}: {}", yansi::Paint::new("New password for").green(), instance_id, yansi::Paint::new(pass).cyan());
                    } else {
                        print_api_response(&payload);
                    }
                    return;
                }
                InstanceCommands::ChangeOs { instance_id, os_id } => {
                    let endpoint = format!("/v1/instances/{}/change-os", instance_id);
                    let payload = serde_json::json!({"osId": os_id});
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    print_api_response(&resp);
                    return;
                }
                InstanceCommands::Resize { instance_id, r#type, product_id, cpu, ram_in_gb, disk_in_gb, bandwidth_in_tb } => {
                    let endpoint = format!("/v1/instances/{}/resize", instance_id);
                    let mut payload = serde_json::json!({"type": r#type});
                    if payload.get("type").and_then(|t| t.as_str()).unwrap_or("") == "FIXED" {
                        if let Some(pid) = product_id {
                            payload["productId"] = serde_json::Value::from(pid);
                        }
                    } else {
                        let mut obj = serde_json::Map::new();
                        if let Some(cpu) = cpu { obj.insert("cpu".into(), serde_json::Value::from(cpu)); }
                        if let Some(ram) = ram_in_gb { obj.insert("ramInGB".into(), serde_json::Value::from(ram)); }
                        if let Some(disk) = disk_in_gb { obj.insert("diskInGB".into(), serde_json::Value::from(disk)); }
                        if let Some(bw) = bandwidth_in_tb { obj.insert("bandwidthInTB".into(), serde_json::Value::from(bw)); }
                        if !obj.is_empty() { payload["resource"] = serde_json::Value::Object(obj); }
                    }
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    print_api_response(&resp);
                    return;
                }
                InstanceCommands::AddTraffic { instance_id, amount } => {
                    let endpoint = format!("/v1/instances/{}/add-traffic", instance_id);
                    let payload = serde_json::json!({"amount": amount});
                    let resp = api_call_wrapper(&state, "POST", &endpoint, Some(payload), None).await;
                    print_api_response(&resp);
                    return;
                }
            }
        }
        Commands::Update { channel } => {
            let channel = match channel.to_lowercase().as_str() {
                "beta" => update::Channel::Beta,
                "alpha" => update::Channel::Alpha,
                "rc" => update::Channel::ReleaseCandidate,
                _ => update::Channel::Stable,
            };

            match update::check_for_update(channel).await {
                Ok(Some(release)) => {
                    println!("\nDownload it from: {}", yansi::Paint::new(release.download_url).underline());
                    println!("(Note: Automatic background updates are coming in Phase 2)");
                }
                Ok(None) => {}
                Err(_) => {
                    process::exit(1);
                }
            }
            return;
        }
    }

}
