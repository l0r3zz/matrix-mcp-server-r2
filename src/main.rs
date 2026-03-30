use std::sync::Arc;
use tracing::info;

mod auth;
mod config;
mod error;
mod matrix;
mod mcp;

use config::Config;
use error::AppError;
use mcp::server::MatrixMcpServer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), AppError> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("matrix-mcp-server-r2 {}", VERSION);
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Matrix MCP Server R2 v{}", VERSION);

    let config = Config::from_env()?;
    config.validate()?;
    info!("Configuration loaded successfully");

    let (client, sync_handle, cleanup_handle) = if config.skip_matrix_init {
        info!("SKIP_MATRIX_INIT is set -- starting without Matrix connection");
        info!("Tools will return errors; use this mode for MCP protocol testing only");
        let client = matrix_sdk::Client::builder()
            .homeserver_url(&config.matrix_homeserver_url)
            .build()
            .await
            .map_err(|e| AppError::MatrixClient(format!("Failed to build stub client: {}", e)))?;
        let sync_handle = tokio::spawn(async {});
        let cleanup_handle = tokio::spawn(async {});
        (client, sync_handle, cleanup_handle)
    } else {
        let client = matrix::create_matrix_client(
            &config.matrix_homeserver_url,
            &config.matrix_user_id,
            &config.matrix_access_token,
        )
        .await?;
        let sync_handle = matrix::client::start_background_sync(client.clone());
        let cache = matrix::ClientCache::new();
        let cleanup_handle = cache.start_cleanup_task();
        cache
            .set(
                &config.matrix_user_id,
                &config.matrix_homeserver_url,
                client.clone(),
            )
            .await;
        (client, sync_handle, cleanup_handle)
    };

    let port = config.port;
    let config = Arc::new(config);

    // Build rmcp StreamableHTTP MCP service
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::tower::{StreamableHttpServerConfig, StreamableHttpService};

    let server_client = client.clone();
    let server_config = config.clone();
    let mcp_service: StreamableHttpService<MatrixMcpServer> = StreamableHttpService::new(
        move || Ok(MatrixMcpServer::new(server_client.clone(), server_config.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    let health_config = config.clone();
    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(move || async move {
                axum::Json(serde_json::json!({
                    "status": "healthy",
                    "server": "matrix-mcp-server-r2",
                    "version": VERSION,
                    "user_id": health_config.matrix_user_id,
                }))
            }),
        )
        .nest_service("/mcp", mcp_service);

    // Apply CORS
    use tower_http::cors::{Any, CorsLayer};
    let cors = if let Some(ref origins) = config.cors_allowed_origins {
        let allowed: Vec<axum::http::HeaderValue> = origins
            .split(',')
            .filter_map(|o| o.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    let app = app
        .layer(cors)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("MCP server listening on port {}", port);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            info!("Shutdown signal received");
            sync_handle.abort();
            cleanup_handle.abort();
        })
        .await
        .map_err(|e| AppError::Unknown(format!("Server error: {}", e)))?;

    info!("Matrix MCP Server R2 shut down");
    Ok(())
}
