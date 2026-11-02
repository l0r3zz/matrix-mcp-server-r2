use matrix_sdk::{
    authentication::matrix::MatrixSession, config::SyncSettings, store::RoomLoadSettings, Client,
    SessionMeta, SessionTokens,
};
use ruma::{OwnedDeviceId, OwnedUserId};
use serde::Deserialize;
use std::time::Duration;
use tracing::info;

use crate::error::{AppError, Result};

/// Body of `GET /_matrix/client/v3/account/whoami`.
///
/// Only the fields needed to restore a session are decoded. The access token
/// is not part of this response and is never logged.
#[derive(Debug, Deserialize)]
struct WhoamiResponse {
    user_id: OwnedUserId,
    device_id: Option<OwnedDeviceId>,
}

/// Create and initialize a Matrix client with the given credentials.
///
/// 1. Build a client pointing at the homeserver.
/// 2. Resolve `device_id` from `/_matrix/client/v3/account/whoami` for the
///    access token. The literal `MCP_SERVER` is not used.
/// 3. Restore the session with that device id.
/// 4. Perform an initial sync so room state is populated.
///
/// This path does not open a crypto store or bootstrap cross-signing. Those
/// steps stay behind the `e2ee` feature in later tasks.
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

    let device_id = resolve_device_id(&client, access_token, &parsed_user).await?;

    let session = MatrixSession {
        meta: SessionMeta {
            user_id: parsed_user,
            device_id,
        },
        tokens: SessionTokens {
            access_token: access_token.to_string(),
            refresh_token: None,
        },
    };

    client
        .matrix_auth()
        .restore_session(session, RoomLoadSettings::default())
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

/// Ask the homeserver which device id is bound to this access token.
///
/// Called before `restore_session`, which requires a device id and will not
/// query `/whoami` itself. The token is sent as a bearer credential and is
/// not written to logs.
async fn resolve_device_id(
    client: &Client,
    access_token: &str,
    expected_user: &OwnedUserId,
) -> Result<OwnedDeviceId> {
    let mut url = client.homeserver();
    url.set_path("/_matrix/client/v3/account/whoami");
    url.set_query(None);
    url.set_fragment(None);

    let response = client
        .http_client()
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::MatrixClient(format!("whoami request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::MatrixClient(format!(
            "whoami returned HTTP {}",
            status
        )));
    }

    let bytes = response.bytes().await.map_err(|e| {
        AppError::MatrixClient(format!("whoami response body could not be read: {}", e))
    })?;
    let body: WhoamiResponse = serde_json::from_slice(&bytes).map_err(|e| {
        AppError::MatrixClient(format!("whoami response was not valid JSON: {}", e))
    })?;

    if &body.user_id != expected_user {
        return Err(AppError::MatrixClient(format!(
            "whoami user_id {} does not match configured user {}",
            body.user_id, expected_user
        )));
    }

    body.device_id.ok_or_else(|| {
        AppError::MatrixClient(
            "whoami response did not include a device_id for this access token".into(),
        )
    })
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
