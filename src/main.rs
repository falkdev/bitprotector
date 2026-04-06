use bitprotector_lib::cli::commands::database::DatabaseCommand;
use bitprotector_lib::cli::commands::drives::DrivesCommand;
use bitprotector_lib::cli::commands::files::FilesCommand;
use bitprotector_lib::cli::commands::folders::FoldersCommand;
use bitprotector_lib::cli::commands::integrity::IntegrityCommand;
use bitprotector_lib::cli::commands::logs::LogsCommand;
use bitprotector_lib::cli::commands::scheduler::SchedulerCommand;
use bitprotector_lib::cli::commands::sync::SyncCommand;
use bitprotector_lib::cli::commands::virtual_paths::VirtualPathsCommand;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// Config file structures (mirrors config/default.toml)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct AppConfig {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    database: DatabaseConfig,
}

#[derive(serde::Deserialize)]
struct ServerConfig {
    host: Option<String>,
    port: Option<u16>,
    tls_cert: Option<String>,
    tls_key: Option<String>,
    rate_limit_rps: Option<usize>,
    jwt_secret: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            tls_cert: None,
            tls_key: None,
            rate_limit_rps: None,
            jwt_secret: None,
        }
    }
}

#[derive(serde::Deserialize, Default)]
struct DatabaseConfig {
    path: Option<String>,
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "bitprotector")]
#[command(about = "Distributed File Mirror and Integrity Protection System")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the database file
    #[arg(
        long,
        default_value = "/var/lib/bitprotector/bitprotector.db",
        global = true
    )]
    db: String,

    /// Path to the configuration file (default: /etc/bitprotector/config.toml)
    #[arg(long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        /// JWT signing secret (hex-encoded or plain text)
        #[arg(long)]
        jwt_secret: Option<String>,
        /// Path to TLS certificate PEM file
        #[arg(long)]
        tls_cert: Option<String>,
        /// Path to TLS private key PEM file
        #[arg(long)]
        tls_key: Option<String>,
        /// Maximum requests per second per IP
        #[arg(long)]
        rate_limit_rps: Option<usize>,
    },
    /// Display system status (for SSH login)
    Status,
    /// Manage drive pairs
    Drives {
        #[command(subcommand)]
        action: DrivesCommand,
    },
    /// Track and manage files
    Files {
        #[command(subcommand)]
        action: FilesCommand,
    },
    /// Run integrity checks and recovery
    Integrity {
        #[command(subcommand)]
        action: IntegrityCommand,
    },
    /// Manage virtual paths and symlinks
    VirtualPaths {
        #[command(subcommand)]
        action: VirtualPathsCommand,
    },
    /// Track folders for auto file discovery
    Folders {
        #[command(subcommand)]
        action: FoldersCommand,
    },
    /// Manage sync queue and scheduled tasks
    Sync {
        #[command(subcommand)]
        action: SyncCommand,
    },
    /// View event logs
    Logs {
        #[command(subcommand)]
        action: LogsCommand,
    },
    /// Manage database backup destinations
    Database {
        #[command(subcommand)]
        action: DatabaseCommand,
    },
    /// Manage scheduler configurations
    Scheduler {
        #[command(subcommand)]
        action: SchedulerCommand,
    },
}

// ---------------------------------------------------------------------------
// Config loading helpers
// ---------------------------------------------------------------------------

/// Load application config from file. Returns a default (empty) config on any error
/// so that CLI flags always remain authoritative.
fn load_app_config(config_path: &str) -> AppConfig {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return AppConfig::default(),
    };
    toml::from_str(&content).unwrap_or_default()
}

fn open_repo(db_path: &str) -> anyhow::Result<bitprotector_lib::db::repository::Repository> {
    let pool = bitprotector_lib::db::repository::create_cli_pool(db_path)?;
    {
        let conn = pool.get()?;
        bitprotector_lib::db::schema::initialize_schema(&conn)?;
    }
    Ok(bitprotector_lib::db::repository::Repository::new(pool))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Skip tracing for the Status command: it runs on every SSH login.
    if !matches!(cli.command, Commands::Status) {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    // ── Load config file as baseline ──────────────────────────────────────────
    let config_path = cli
        .config
        .as_deref()
        .unwrap_or("/etc/bitprotector/config.toml");
    let file_cfg = load_app_config(config_path);

    // Resolve database path: CLI flag > config file > hardcoded default
    let db_path = if cli.db != "/var/lib/bitprotector/bitprotector.db" {
        // Explicitly set on CLI
        cli.db.clone()
    } else if let Some(ref p) = file_cfg.database.path {
        p.clone()
    } else {
        cli.db.clone()
    };

    match cli.command {
        Commands::Serve {
            host,
            port,
            jwt_secret,
            tls_cert,
            tls_key,
            rate_limit_rps,
        } => {
            // CLI flag > config file > hardcoded default
            let resolved_host = host
                .or_else(|| file_cfg.server.host.clone())
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let resolved_port = port.or(file_cfg.server.port).unwrap_or(8443);
            let resolved_secret = jwt_secret
                .or_else(|| file_cfg.server.jwt_secret.clone())
                .unwrap_or_else(|| "change-me-in-production".to_string());
            let resolved_tls_cert = tls_cert.or_else(|| file_cfg.server.tls_cert.clone());
            let resolved_tls_key = tls_key.or_else(|| file_cfg.server.tls_key.clone());
            let resolved_rate_limit = rate_limit_rps
                .or(file_cfg.server.rate_limit_rps)
                .unwrap_or(100);

            tracing::info!(
                "Starting BitProtector server on {}:{}",
                resolved_host,
                resolved_port
            );
            bitprotector_lib::api::server::run_server(
                &resolved_host,
                resolved_port,
                &db_path,
                resolved_secret.into_bytes(),
                resolved_tls_cert.as_deref(),
                resolved_tls_key.as_deref(),
                resolved_rate_limit,
            )
            .await?;
        }
        Commands::Status => {
            bitprotector_lib::cli::ssh_status::print_status(&db_path);
        }
        Commands::Drives { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::drives::handle(action, &repo)?;
        }
        Commands::Files { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::files::handle(action, &repo)?;
        }
        Commands::Integrity { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::integrity::handle(action, &repo)?;
        }
        Commands::VirtualPaths { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::virtual_paths::handle(action, &repo)?;
        }
        Commands::Folders { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::folders::handle(action, &repo)?;
        }
        Commands::Sync { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::sync::handle(action, &repo)?;
        }
        Commands::Logs { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::logs::handle(action, &repo)?;
        }
        Commands::Database { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::database::handle(action, &repo)?;
        }
        Commands::Scheduler { action } => {
            let repo = open_repo(&db_path)?;
            bitprotector_lib::cli::commands::scheduler::handle(action, &repo)?;
        }
    }

    Ok(())
}
