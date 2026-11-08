use std::sync::Arc;
use tracing::info;

mod auth;
mod bot;
mod config;
mod error;
mod health;
mod matrix;
mod mcp;

use config::Config;
use error::AppError;
use mcp::server::MatrixMcpServer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `true` when the local device is cross-signed by its owner.
///
/// A store lookup failure leaves the flag false. `/health` still returns 200.
async fn health_cross_signed(client: &matrix_sdk::Client) -> bool {
    #[cfg(feature = "e2ee")]
    {
        return client
            .encryption()
            .get_own_device()
            .await
            .ok()
            .flatten()
            .is_some_and(|device| device.is_cross_signed_by_owner());
    }
    #[cfg(not(feature = "e2ee"))]
    {
        let _ = client;
        false
    }
}

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

    let (client, sync_handle, cleanup_handle, _store_lock) = if config.skip_matrix_init {
        info!("SKIP_MATRIX_INIT is set -- starting without Matrix connection");
        info!("Tools will return errors; use this mode for MCP protocol testing only");
        let client = matrix_sdk::Client::builder()
            .homeserver_url(&config.matrix_homeserver_url)
            .build()
            .await
            .map_err(|e| AppError::MatrixClient(format!("Failed to build stub client: {}", e)))?;
        let sync_handle = tokio::spawn(async {});
        let cleanup_handle = tokio::spawn(async {});
        (client, sync_handle, cleanup_handle, None)
    } else {
        let (store_path, recovery_key_file, uiaa_password_file) = if config.e2ee_enabled {
            (
                config
                    .crypto_store_path
                    .as_deref()
                    .map(std::path::Path::new),
                config
                    .recovery_key_file
                    .as_deref()
                    .map(std::path::Path::new),
                config
                    .uiaa_password_file
                    .as_deref()
                    .map(std::path::Path::new),
            )
        } else {
            (None, None, None)
        };
        let initialized = matrix::create_matrix_client(
            &config.matrix_homeserver_url,
            &config.matrix_user_id,
            &config.matrix_access_token,
            store_path,
            recovery_key_file,
            uiaa_password_file,
        )
        .await?;
        let client = initialized.client;
        if config.bot_enabled {
            bot::attach(
                &client,
                &bot::Session {
                    user_id: config.matrix_user_id.clone(),
                    display_name: config.bot_display_name.clone(),
                    identity: config.agent_identity.clone(),
                    state_dir: config.bot_state_dir.clone().into(),
                    a0_api_url: config.a0_api_url.clone(),
                    amctl_url: config.amctl_url.clone(),
                    amctl_api_key: config.amctl_api_key.clone(),
                    trusted_senders: config.trusted_senders.clone(),
                },
            )
            .await;
        }
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
        (client, sync_handle, cleanup_handle, initialized.store_lock)
    };

    let port = config.port;
    let config = Arc::new(config);

    // Build rmcp StreamableHTTP MCP service
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::tower::{
        StreamableHttpServerConfig, StreamableHttpService,
    };

    let server_client = client.clone();
    let server_config = config.clone();
    let mcp_service: StreamableHttpService<MatrixMcpServer> = StreamableHttpService::new(
        move || {
            Ok(MatrixMcpServer::new(
                server_client.clone(),
                server_config.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    let health_config = config.clone();
    let health_client = client.clone();
    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(move || {
                let health_config = health_config.clone();
                let health_client = health_client.clone();
                async move {
                    let (e2ee, _) = health::health_flags(
                        health_config.e2ee_enabled,
                        health_config.skip_matrix_init,
                        false,
                    );
                    let cross_signed = e2ee && health_cross_signed(&health_client).await;
                    let bot_enabled = health::bot_enabled_flag(
                        health_config.bot_enabled,
                        bot::handlers_registered(),
                    );
                    axum::Json(health::health_response(
                        &health_config.matrix_user_id,
                        VERSION,
                        e2ee,
                        cross_signed,
                        bot_enabled,
                    ))
                }
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
