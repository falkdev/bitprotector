use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

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
    }

    Ok(())
}
