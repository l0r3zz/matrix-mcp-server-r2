use matrix_sdk::{
    authentication::matrix::{MatrixSession, MatrixSessionTokens},
    config::SyncSettings,
    Client, SessionMeta,
};
use ruma::{OwnedDeviceId, OwnedUserId};
use std::time::Duration;
use tracing::info;

use crate::error::{AppError, Result};

/// Create and initialize a Matrix client with the given credentials.
///
/// Mirrors the TS `createMatrixClient` flow:
/// 1. Build client pointing at homeserver
/// 2. Restore session with the provided access token
/// 3. Perform an initial sync so room state is populated
pub async fn create_matrix_client(
    homeserver_url: &str,
    user_id: &str,
    access_token: &str,
) -> Result<Client> {
    let parsed_user = OwnedUserId::try_from(user_id)
        .map_err(|e| AppError::InvalidParameter(format!("Invalid user ID '{}': {}", user_id, e)))?;

    let client = Client::builder()
        .homeserver_url(homeserver_url)
        .build()
        .await
        .map_err(|e| AppError::MatrixClient(format!("Failed to build client: {}", e)))?;

    let session = MatrixSession {
        meta: SessionMeta {
            user_id: parsed_user,
            device_id: OwnedDeviceId::from("MCP_SERVER"),
        },
        tokens: MatrixSessionTokens {
            access_token: access_token.to_string(),
            refresh_token: None,
        },
    };

    client
        .matrix_auth()
        .restore_session(session)
        .await
        .map_err(|e| AppError::MatrixClient(format!("Failed to restore session: {}", e)))?;

    info!("Performing initial sync for {}", user_id);
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(30)))
        .await
        .map_err(|e| AppError::MatrixClient(format!("Initial sync failed: {}", e)))?;

    info!("Matrix client ready for {}", user_id);
    Ok(client)
}

/// Start a background sync loop for the given client.
pub fn start_background_sync(client: Client) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let settings = SyncSettings::default().timeout(Duration::from_secs(30));
        loop {
            match client.sync_once(settings.clone()).await {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Background sync error: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}
