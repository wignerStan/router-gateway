//! Gateway CLI - Management utility for the LLM Gateway
//!
//! This CLI provides commands for managing the gateway, including:
//! - Health checks
//! - Configuration validation
//! - Model registry operations

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check gateway health status
    Health {
        /// Gateway URL
        #[arg(short, long, default_value = "http://localhost:3000")]
        url: String,
    },
    /// List available models
    Models {
        /// Gateway URL
        #[arg(short, long, default_value = "http://localhost:3000")]
        url: String,
    },
    /// Validate configuration file
    Validate {
        /// Path to configuration file
        #[arg(short, long)]
        config: String,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    match args.command {
        Commands::Health { url } => {
            info!("Checking health at {}", url);
            // TODO: Implement health check
            println!("Health check: OK (not implemented)");
        },
        Commands::Models { url } => {
            info!("Fetching models from {}", url);
            // TODO: Implement model listing
            println!("Models: (not implemented)");
        },
        Commands::Validate { config } => {
            info!("Validating configuration: {}", config);
            // TODO: Implement config validation
            println!("Configuration valid: (not implemented)");
        },
    }
}
