use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use bitprotector_lib::cli::commands::drives::DrivesCommand;

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
    },
    /// Display system status (for SSH login)
    Status,
    /// Manage drive pairs
    Drives {
        #[command(subcommand)]
        action: DrivesCommand,
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
        Commands::Serve { host, port } => {
            tracing::info!("Starting BitProtector server on {}:{}", host, port);
            bitprotector_lib::api::server::run_server(&host, port).await?;
        }
        Commands::Status => {
            bitprotector_lib::cli::ssh_status::print_status(&cli.db);
        }
        Commands::Drives { action } => {
            let repo = open_repo(&cli.db)?;
            bitprotector_lib::cli::commands::drives::handle(action, &repo)?;
        }
    }

    Ok(())
}
