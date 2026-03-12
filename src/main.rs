use std::sync::Arc;
use tracing::{info, warn};

mod config;
mod error;
mod mcp;
mod matrix;
mod tools;

use config::Config;
use error::AppError;
use mcp::server::MatrixMcpServer;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Matrix MCP Server R2");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Create and run the MCP server
    let server = MatrixMcpServer::new(config).await?;
    
    info!("Matrix MCP Server R2 is running");
    server.run().await?;

    Ok(())
}
