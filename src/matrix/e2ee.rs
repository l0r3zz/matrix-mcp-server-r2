//! Cross-signing bootstrap on top of `matrix-sdk` 0.19.
//!
//! Startup and the `bootstrap-e2ee` tool share this path. Historical key
//! backup download runs on a background task so cold start does not wait for
//! the whole backup. `forceRecreate` never calls `reset_cross_signing` or
//! `reset_identity`: the server-side identity stays, and a fresh store still
//! needs a recovery secret when that identity already exists.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use matrix_sdk::encryption::backups::BackupState;
use matrix_sdk::ruma::api::client::uiaa::{self, AuthData};
use matrix_sdk::Client;
use serde::Serialize;
use tracing::warn;

use super::secrets::{
    require_cross_signed_policy, resolve_recovery_secret, write_recovery_key_file,
    RecoverySecretRequest,
};
use crate::error::{AppError, IdentityExistsError, Result, UiaaRequiredError};

/// Arguments supplied by startup (all empty) or by `bootstrap-e2ee`.
#[derive(Debug, Default)]
pub struct BootstrapRequest {
    /// Omitted means `cross-signed`.
    pub trust_policy: Option<String>,
    /// Secret-manager reference. Unresolvable until a backend is configured.
    pub backup_passphrase_ref: Option<String>,
    /// Environment variable name. Not the secret itself.
    pub backup_passphrase_env_var: Option<String>,
    /// Rotate nothing on the server. The open device id stays the whoami id.
    pub force_recreate: bool,
}

impl BootstrapRequest {
    /// Startup auto-bootstrap: file and UIAA path come from config, not from
    /// an MCP argument.
    pub fn startup() -> Self {
        Self::default()
    }
}

/// `bootstrap-e2ee` response. No field holds a recovery secret or password.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapReport {
    /// Device id bound to the restored session.
    pub device_id: String,
    /// Public Ed25519 fingerprint of this device.
    pub identity_key: String,
    /// This device is signed by the account's cross-signing identity.
    pub cross_signed: bool,
    /// Olm is active for this client.
    pub olm_enabled: bool,
    /// `ready` once the SQLite store is the one this process opened.
    pub crypto_store_state: String,
    /// One-time keys uploaded in this call. The 0.19 public API does not
    /// expose a count, so this stays 0.
    pub uploaded_one_time_keys: usize,
    /// A server-side key backup exists.
    pub backup_configured: bool,
    /// `none`, `env_var`, or `recovery_key_file`.
    pub resolved_backup_source: String,
    /// `not_started`, `running`, `done`, or `failed`.
    pub key_backup_restore: String,
    /// Always `cross-signed` when this report is returned.
    pub trust_policy: String,
    /// Operator note. Never a secret.
    pub notes: String,
}

struct E2eeRuntime {
    restore: Mutex<String>,
    source: Mutex<String>,
    notes: Mutex<String>,
}

impl E2eeRuntime {
    fn new() -> Self {
        Self {
            restore: Mutex::new("not_started".to_string()),
            source: Mutex::new("none".to_string()),
            notes: Mutex::new(String::new()),
        }
    }

    fn set_restore(&self, value: &str) {
        *self.restore.lock().unwrap_or_else(|err| err.into_inner()) = value.to_string();
    }

    fn restore(&self) -> String {
        self.restore
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    fn set_source(&self, value: &str) {
        *self.source.lock().unwrap_or_else(|err| err.into_inner()) = value.to_string();
    }

    fn source(&self) -> String {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    fn set_notes(&self, value: &str) {
        *self.notes.lock().unwrap_or_else(|err| err.into_inner()) = value.to_string();
    }

    fn notes(&self) -> String {
        self.notes
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

fn runtime() -> &'static E2eeRuntime {
    static RUNTIME: OnceLock<E2eeRuntime> = OnceLock::new();
    RUNTIME.get_or_init(E2eeRuntime::new)
}

/// Backup source and restore progress for `get-encryption-status`.
///
/// A status read before bootstrap reports `none` and `not_started`.
pub fn bootstrap_progress() -> BootstrapProgress {
    let state = runtime();
    BootstrapProgress {
        backup_source: state.source(),
        key_backup_restore: state.restore(),
    }
}

/// Snapshot of the bootstrap runtime. No field holds a secret.
pub struct BootstrapProgress {
    /// `none`, `env_var`, or `recovery_key_file`.
    pub backup_source: String,
    /// `not_started`, `running`, `done`, or `failed`.
    pub key_backup_restore: String,
}

/// Unlock Secret Storage or create a cross-signing identity, then schedule
/// key-backup download.
///
/// The returned report is built before the download task finishes.
pub async fn bootstrap_e2ee(
    client: &Client,
    recovery_key_file: Option<&Path>,
    uiaa_password_file: Option<&Path>,
    request: BootstrapRequest,
) -> Result<BootstrapReport> {
    require_cross_signed_policy(request.trust_policy.as_deref())?;

    let state = runtime();
    if request.force_recreate {
        // The access token is already bound to the whoami device id. Minting
        // a second Olm account under that id is the failure mode the spec
        // forbids. Re-import the server identity instead of resetting it.
        state.set_notes(
            "forceRecreate left the server cross-signing identity unchanged and kept the whoami device id",
        );
    } else {
        state.set_notes("");
    }

    let user_id = client
        .user_id()
        .ok_or_else(|| AppError::MatrixClient("session has no user id".into()))?
        .to_owned();

    let identity = client
        .encryption()
        .request_user_identity(&user_id)
        .await
        .map_err(|_| {
            AppError::MatrixClient("failed to query the account cross-signing identity".into())
        })?;

    let resolved = resolve_recovery_secret(RecoverySecretRequest {
        backup_passphrase_ref: request.backup_passphrase_ref.as_deref(),
        backup_passphrase_env_var: request.backup_passphrase_env_var.as_deref(),
        recovery_key_file,
    })?;
    state.set_source(resolved.source().as_str());

    match (identity.is_some(), resolved.into_secret()) {
        (true, None) => {
            return Err(IdentityExistsError::new(
                "cross-signing identity already exists and no recovery secret is available",
            )
            .into());
        }
        (_, Some(secret)) => {
            let unlocked = client.encryption().recovery().recover(&secret).await;
            if unlocked.is_err() {
                warn!("secret storage unlock failed");
            }
            drop(secret);
            unlocked
                .map_err(|_| AppError::MatrixClient("failed to unlock secret storage".into()))?;
        }
        (false, None) => {
            create_identity(client, uiaa_password_file, recovery_key_file).await?;
            state.set_source("recovery_key_file");
        }
    }

    state.set_restore("running");
    let background = client.clone();
    tokio::spawn(async move {
        let failed = download_joined_room_keys(&background).await.is_err();
        if failed {
            warn!("historical key backup restore failed");
            runtime().set_restore("failed");
        } else {
            runtime().set_restore("done");
        }
    });

    report(client, state).await
}

async fn create_identity(
    client: &Client,
    uiaa_password_file: Option<&Path>,
    recovery_key_file: Option<&Path>,
) -> Result<()> {
    let path = recovery_key_file
        .ok_or_else(|| AppError::Config("MATRIX_RECOVERY_KEY_FILE is not configured".into()))?;
    upload_cross_signing(client, uiaa_password_file).await?;
    let recovery_key = client
        .encryption()
        .recovery()
        .enable()
        .await
        .map_err(|_| AppError::MatrixClient("failed to create secret storage".into()))?;
    let wrote = write_recovery_key_file(path, &recovery_key);
    drop(recovery_key);
    wrote.map_err(|_| AppError::MatrixClient("failed to write the recovery key file".into()))?;
    Ok(())
}

async fn upload_cross_signing(client: &Client, uiaa_password_file: Option<&Path>) -> Result<()> {
    match client.encryption().bootstrap_cross_signing(None).await {
        Ok(()) => Ok(()),
        Err(error) => {
            if error.as_uiaa_response().is_none() {
                warn!("cross-signing upload failed");
                return Err(AppError::MatrixClient("cross-signing upload failed".into()));
            }
            let session = error
                .as_uiaa_response()
                .and_then(|info| info.session.clone());
            let password = read_uiaa_password(uiaa_password_file)?;
            let user_id = client
                .user_id()
                .ok_or_else(|| AppError::MatrixClient("session has no user id".into()))?
                .to_owned();
            let mut password_auth = uiaa::Password::new(user_id.into(), password);
            password_auth.session = session;
            match client
                .encryption()
                .bootstrap_cross_signing(Some(AuthData::Password(password_auth)))
                .await
            {
                Ok(()) => Ok(()),
                Err(retry) if retry.as_uiaa_response().is_some() => Err(UiaaRequiredError::new(
                    "homeserver rejected UIAA for the cross-signing upload",
                )
                .into()),
                Err(_) => {
                    warn!("cross-signing upload failed after UIAA");
                    Err(AppError::MatrixClient("cross-signing upload failed".into()))
                }
            }
        }
    }
}

fn read_uiaa_password(path: Option<&Path>) -> Result<String> {
    let Some(path) = path else {
        return Err(UiaaRequiredError::new("MATRIX_UIAA_PASSWORD_FILE is unset").into());
    };
    if !path.exists() {
        return Err(UiaaRequiredError::new("MATRIX_UIAA_PASSWORD_FILE is unreadable").into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(
                UiaaRequiredError::new("MATRIX_UIAA_PASSWORD_FILE must be mode 0600").into(),
            );
        }
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| UiaaRequiredError::new("MATRIX_UIAA_PASSWORD_FILE is unreadable"))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(UiaaRequiredError::new("MATRIX_UIAA_PASSWORD_FILE is empty").into());
    }
    Ok(trimmed.to_string())
}

async fn download_joined_room_keys(client: &Client) -> Result<()> {
    let backups = client.encryption().backups();
    let exists = backups
        .fetch_exists_on_server()
        .await
        .map_err(|_| AppError::MatrixClient("failed to query key backup".into()))?;
    if !exists {
        return Ok(());
    }
    for room in client.joined_rooms() {
        backups
            .download_room_keys_for_room(room.room_id())
            .await
            .map_err(|_| AppError::MatrixClient("failed to download room keys".into()))?;
    }
    Ok(())
}

async fn report(client: &Client, state: &E2eeRuntime) -> Result<BootstrapReport> {
    let device = client
        .encryption()
        .get_own_device()
        .await
        .map_err(|_| AppError::MatrixClient("failed to read the local device".into()))?;
    let identity_key = device
        .as_ref()
        .and_then(|device| device.ed25519_key())
        .map(|key| key.to_base64())
        .unwrap_or_default();
    let cross_signed = device
        .as_ref()
        .map(|device| device.is_cross_signed_by_owner())
        .unwrap_or(false);
    let backup_configured = matches!(
        client.encryption().backups().state(),
        BackupState::Creating
            | BackupState::Enabling
            | BackupState::Resuming
            | BackupState::Enabled
            | BackupState::Downloading
    );
    Ok(BootstrapReport {
        device_id: client
            .device_id()
            .map(|id| id.to_string())
            .unwrap_or_default(),
        identity_key,
        cross_signed,
        olm_enabled: device.is_some(),
        crypto_store_state: "ready".to_string(),
        uploaded_one_time_keys: 0,
        backup_configured,
        resolved_backup_source: state.source(),
        key_backup_restore: state.restore(),
        trust_policy: "cross-signed".to_string(),
        notes: state.notes(),
    })
}
