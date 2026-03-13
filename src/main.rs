use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use bitprotector_lib::cli::commands::drives::DrivesCommand;
use bitprotector_lib::cli::commands::files::FilesCommand;
use bitprotector_lib::cli::commands::integrity::IntegrityCommand;
use bitprotector_lib::cli::commands::virtual_paths::VirtualPathsCommand;
use bitprotector_lib::cli::commands::folders::FoldersCommand;
use bitprotector_lib::cli::commands::sync::SyncCommand;
use bitprotector_lib::cli::commands::logs::LogsCommand;
use bitprotector_lib::cli::commands::database::DatabaseCommand;

#[derive(Parser)]
#[command(name = "bitprotector")]
#[command(about = "Distributed File Mirror and Integrity Protection System")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the database file
    #[arg(long, default_value = "/var/lib/bitprotector/bitprotector.db", global = true)]
    db: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value_t = 8443)]
        port: u16,
        /// JWT signing secret (hex-encoded or plain text)
        #[arg(long, default_value = "change-me-in-production")]
        jwt_secret: String,
        /// Path to TLS certificate PEM file
        #[arg(long)]
        tls_cert: Option<String>,
        /// Path to TLS private key PEM file
        #[arg(long)]
        tls_key: Option<String>,
        /// Maximum requests per second per IP
        #[arg(long, default_value_t = 100)]
        rate_limit_rps: usize,
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
}

fn open_repo(db_path: &str) -> anyhow::Result<bitprotector_lib::db::repository::Repository> {
    let pool = bitprotector_lib::db::repository::create_pool(db_path)?;
    let conn = pool.get()?;
    bitprotector_lib::db::schema::initialize_schema(&*conn)?;
    drop(conn);
    Ok(bitprotector_lib::db::repository::Repository::new(pool))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port, jwt_secret, tls_cert, tls_key, rate_limit_rps } => {
            tracing::info!("Starting BitProtector server on {}:{}", host, port);
            bitprotector_lib::api::server::run_server(
                &host,
                port,
                &cli.db,
                jwt_secret.into_bytes(),
                tls_cert.as_deref(),
                tls_key.as_deref(),
                rate_limit_rps,
            ).await?;
        }
        Commands::Status => {
            bitprotector_lib::cli::ssh_status::print_status(&cli.db);
        }
        Commands::Drives { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::drives::handle(action, &repo)?;
        }
        Commands::Files { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::files::handle(action, &repo)?;
        }
        Commands::Integrity { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::integrity::handle(action, &repo)?;
        }
        Commands::VirtualPaths { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::virtual_paths::handle(action, &repo)?;
        }
        Commands::Folders { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::folders::handle(action, &repo)?;
        }
        Commands::Sync { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::sync::handle(action, &repo)?;
        }
        Commands::Logs { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::logs::handle(action, &repo)?;
        }
        Commands::Database { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::database::handle(action, &repo)?;
        }
    }

    Ok(())
}
