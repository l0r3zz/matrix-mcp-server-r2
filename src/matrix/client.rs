#[cfg(feature = "e2ee")]
use matrix_sdk::encryption::{BackupDownloadStrategy, EncryptionSettings};
use matrix_sdk::{
    authentication::matrix::MatrixSession, config::SyncSettings, store::RoomLoadSettings, Client,
    SessionMeta, SessionTokens,
};
#[cfg(feature = "e2ee")]
use matrix_sdk_base::crypto::{CollectStrategy, DecryptionSettings, TrustRequirement};
use ruma::{OwnedDeviceId, OwnedUserId};
use serde::Deserialize;
use std::fs::OpenOptions;
#[cfg(any(test, feature = "e2ee"))]
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;

use crate::error::{AppError, KeyStoreUnavailableError, Result};

/// Body of `GET /_matrix/client/v3/account/whoami`.
///
/// Only the fields needed to restore a session are decoded. The access token
/// is not part of this response and is never logged.
#[derive(Debug, Deserialize)]
struct WhoamiResponse {
    user_id: OwnedUserId,
    device_id: Option<OwnedDeviceId>,
}

/// Client plus the store lock that must stay alive for the process lifetime.
pub struct InitializedMatrixClient {
    pub client: Client,
    /// `Some` only when the `e2ee` feature is on and a crypto store path was
    /// requested. Dropping the guard releases the exclusive lock.
    pub store_lock: Option<CryptoStoreGuard>,
}

/// Exclusive lock on `$CRYPTO_STORE_PATH/.store.lock`.
///
/// The lock is the open file descriptor. The kernel releases it when this
/// value is dropped or the process exits.
pub struct CryptoStoreGuard {
    _lock: std::fs::File,
}

/// Create and initialize a Matrix client with the given credentials.
///
/// 1. When `crypto_store_path` is set (requires the `e2ee` feature), create
///    the directory at mode `0700` if needed and take an exclusive lock.
/// 2. Build a client. With a store path, the bundled SQLite store is opened
///    at that directory.
/// 3. Resolve `device_id` from `/_matrix/client/v3/account/whoami`. The
///    literal `MCP_SERVER` is not used. With a store path, that id is read
///    from or written to `$CRYPTO_STORE_PATH/.device-id`.
/// 4. Restore the session and run an initial sync.
/// 5. When a crypto store is open, run cross-signing bootstrap. Historical
///    key backup download continues after this function returns.
pub async fn create_matrix_client(
    homeserver_url: &str,
    user_id: &str,
    access_token: &str,
    crypto_store_path: Option<&Path>,
    recovery_key_file: Option<&Path>,
    uiaa_password_file: Option<&Path>,
) -> Result<InitializedMatrixClient> {
    let parsed_user = OwnedUserId::try_from(user_id)
        .map_err(|e| AppError::InvalidParameter(format!("Invalid user ID '{}': {}", user_id, e)))?;

    let builder = Client::builder().homeserver_url(homeserver_url);

    #[cfg(feature = "e2ee")]
    let (builder, store_lock) = match crypto_store_path {
        Some(path) => {
            let guard = open_locked_crypto_store(path)?;
            // No sqlite passphrase: the directory mode is the at-rest control.
            // The recovery key is a separate Secret Storage secret.
            info!("Opened SQLite crypto store");
            (
                apply_cross_signed_trust(builder.sqlite_store(path, None)),
                Some(guard),
            )
        }
        None => (apply_cross_signed_trust(builder), None),
    };
    #[cfg(not(feature = "e2ee"))]
    let store_lock = {
        if crypto_store_path.is_some() {
            return Err(AppError::Config(
                "crypto store requested but this binary was built without --features e2ee".into(),
            ));
        }
        None
    };

    let client = builder.build().await.map_err(|e| {
        if crypto_store_path.is_some() {
            AppError::KeyStoreUnavailable(KeyStoreUnavailableError::new(format!(
                "failed to open crypto store: {e}"
            )))
        } else {
            AppError::MatrixClient(format!("Failed to build client: {e}"))
        }
    })?;

    let whoami_device = resolve_device_id(&client, access_token, &parsed_user).await?;
    let device_id = match crypto_store_path {
        Some(path) => load_or_init_device_id(path, &whoami_device)?,
        None => whoami_device,
    };

    let session = MatrixSession {
        meta: SessionMeta {
            user_id: parsed_user,
            device_id,
        },
        // SessionTokens owns the token. The SDK does not borrow the caller's str.
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

    #[cfg(feature = "e2ee")]
    if crypto_store_path.is_some() {
        super::e2ee::bootstrap_e2ee(
            &client,
            recovery_key_file,
            uiaa_password_file,
            super::e2ee::BootstrapRequest::startup(),
        )
        .await?;
    } else {
        let _ = (recovery_key_file, uiaa_password_file);
    }
    #[cfg(not(feature = "e2ee"))]
    {
        let _ = (recovery_key_file, uiaa_password_file);
    }

    info!("Matrix client ready for {}", user_id);
    Ok(InitializedMatrixClient { client, store_lock })
}

/// Share Megolm keys only with cross-signed devices, and decrypt only events
/// from cross-signed senders. Auto-bootstrap stays off so this process, not
/// the SDK default, decides when an identity is created.
#[cfg(feature = "e2ee")]
fn apply_cross_signed_trust(builder: matrix_sdk::ClientBuilder) -> matrix_sdk::ClientBuilder {
    builder
        .with_room_key_recipient_strategy(CollectStrategy::IdentityBasedStrategy)
        .with_decryption_settings(DecryptionSettings {
            sender_device_trust_requirement: TrustRequirement::CrossSigned,
        })
        .with_encryption_settings(EncryptionSettings {
            auto_enable_cross_signing: false,
            auto_enable_backups: false,
            backup_download_strategy: BackupDownloadStrategy::Manual,
        })
}

/// Create the store directory at mode 0700 when it is missing, then lock
/// `.store.lock` so a second process cannot open the same store.
#[cfg(any(test, feature = "e2ee"))]
fn open_locked_crypto_store(path: &Path) -> Result<CryptoStoreGuard> {
    if path.exists() {
        if !path.is_dir() {
            return Err(store_unavailable(
                "CRYPTO_STORE_PATH exists and is not a directory",
            ));
        }
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path)
                .map_err(|e| {
                    store_unavailable(format!("could not create crypto store directory: {e}"))
                })?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(path).map_err(|e| {
                store_unavailable(format!("could not create crypto store directory: {e}"))
            })?;
        }
    }

    let lock_path = path.join(".store.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&lock_path)
        .map_err(|e| store_unavailable(format!("could not open crypto store lock file: {e}")))?;

    if let Err(error) = try_exclusive_lock(&file) {
        if error.kind() == ErrorKind::WouldBlock {
            return Err(store_unavailable(
                "crypto store is already locked by another process",
            ));
        }
        return Err(store_unavailable(format!(
            "could not lock crypto store: {error}"
        )));
    }

    Ok(CryptoStoreGuard { _lock: file })
}

/// Non-blocking exclusive `flock(2)`.
///
/// `unsafe` is required because Rust has no flock wrapper. The constants are
/// the Linux values (`LOCK_EX` = 2, `LOCK_NB` = 4). The descriptor comes from
/// a live `File`, and the call does not dereference caller memory.
#[cfg(all(unix, any(test, feature = "e2ee")))]
fn try_exclusive_lock(file: &std::fs::File) -> std::io::Result<()> {
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    let fd = std::os::fd::AsRawFd::as_raw_fd(file);
    let rc = unsafe { flock(fd, LOCK_EX | LOCK_NB) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(unix), any(test, feature = "e2ee")))]
fn try_exclusive_lock(_file: &std::fs::File) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "crypto store locking requires flock(2)",
    ))
}

/// Use `$CRYPTO_STORE_PATH/.device-id` when it already matches whoami.
/// Otherwise write the whoami device id. Never seed the file with `MCP_SERVER`.
fn load_or_init_device_id(store: &Path, whoami_device: &OwnedDeviceId) -> Result<OwnedDeviceId> {
    let path = device_id_path(store);
    if path.exists() {
        let stored = std::fs::read_to_string(&path)
            .map_err(|e| store_unavailable(format!("could not read .device-id: {e}")))?;
        let stored = stored.trim();
        if stored.is_empty() {
            return Err(store_unavailable(".device-id is empty"));
        }
        if stored == "MCP_SERVER" {
            return Err(store_unavailable(
                ".device-id must not be the literal MCP_SERVER",
            ));
        }
        let stored_id = OwnedDeviceId::from(stored);
        if &stored_id != whoami_device {
            return Err(store_unavailable(format!(
                ".device-id {stored_id} does not match whoami device_id {whoami_device}"
            )));
        }
        return Ok(stored_id);
    }

    write_device_id(&path, whoami_device)?;
    // The session takes ownership. `whoami_device` is borrowed by the file write.
    Ok(whoami_device.clone())
}

fn device_id_path(store: &Path) -> PathBuf {
    store.join(".device-id")
}

fn write_device_id(path: &Path, device_id: &OwnedDeviceId) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| store_unavailable(format!("could not create .device-id: {e}")))?;
    writeln!(file, "{device_id}")
        .map_err(|e| store_unavailable(format!("could not write .device-id: {e}")))?;
    Ok(())
}

fn store_unavailable(message: impl Into<String>) -> AppError {
    AppError::KeyStoreUnavailable(KeyStoreUnavailableError::new(message))
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
///
/// `sync_once` keeps room state current. With the `e2ee` feature, the same
/// call also delivers to-device events and crypto key updates through the
/// SQLite store opened for this client.
pub fn start_background_sync(client: Client) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut settings = SyncSettings::default().timeout(Duration::from_secs(30));
        loop {
            match client.sync_once(settings.clone()).await {
                Ok(response) => {
                    settings = settings.token(response.next_batch);
                }
                Err(e) => {
                    tracing::warn!("Background sync error: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_store() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("matrix-mcp-store-{}-{}", std::process::id(), nanos));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn device_id_file_is_created_from_whoami_and_reread() {
        let dir = temp_store();
        std::fs::create_dir_all(&dir).unwrap();
        let device = OwnedDeviceId::from("DEVICE_FROM_WHOAMI");
        let first = load_or_init_device_id(&dir, &device).unwrap();
        assert_eq!(first, device);
        let mode = std::fs::metadata(device_id_path(&dir))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let second = load_or_init_device_id(&dir, &device).unwrap();
        assert_eq!(second, device);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_store_paths_yield_two_device_ids() {
        let first_dir = temp_store();
        let second_dir = temp_store();
        std::fs::create_dir_all(&first_dir).unwrap();
        std::fs::create_dir_all(&second_dir).unwrap();
        let first_id = OwnedDeviceId::from("DEVICE_PATH_A");
        let second_id = OwnedDeviceId::from("DEVICE_PATH_B");
        assert_eq!(
            load_or_init_device_id(&first_dir, &first_id).unwrap(),
            first_id
        );
        assert_eq!(
            load_or_init_device_id(&second_dir, &second_id).unwrap(),
            second_id
        );
        assert_ne!(
            std::fs::read_to_string(device_id_path(&first_dir))
                .unwrap()
                .trim(),
            std::fs::read_to_string(device_id_path(&second_dir))
                .unwrap()
                .trim()
        );
        assert_eq!(
            load_or_init_device_id(&first_dir, &first_id).unwrap(),
            first_id
        );
        let _ = std::fs::remove_dir_all(&first_dir);
        let _ = std::fs::remove_dir_all(&second_dir);
    }

    #[test]
    fn device_id_file_rejects_whoami_mismatch_and_mcp_server_literal() {
        let dir = temp_store();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(device_id_path(&dir), "STORED_DEVICE\n").unwrap();
        let other = OwnedDeviceId::from("OTHER_DEVICE");
        let err = load_or_init_device_id(&dir, &other).unwrap_err();
        assert!(matches!(err, AppError::KeyStoreUnavailable(_)));

        std::fs::write(device_id_path(&dir), "MCP_SERVER\n").unwrap();
        let literal = OwnedDeviceId::from("MCP_SERVER");
        let err = load_or_init_device_id(&dir, &literal).unwrap_err();
        assert!(matches!(err, AppError::KeyStoreUnavailable(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_process_cannot_lock_the_same_store() {
        if let Ok(dir) = std::env::var("MATRIX_MCP_LOCK_CHILD") {
            match open_locked_crypto_store(Path::new(&dir)) {
                Ok(_guard) => std::process::exit(0),
                Err(AppError::KeyStoreUnavailable(_)) => std::process::exit(11),
                Err(_) => std::process::exit(12),
            }
        }

        let dir = temp_store();
        let guard = open_locked_crypto_store(&dir).expect("first lock");
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        assert_eq!(probe_lock(&dir), 11, "second process should fail the lock");
        drop(guard);
        assert_eq!(
            probe_lock(&dir),
            0,
            "lock should be free after the holder drops"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn probe_lock(dir: &Path) -> i32 {
        let status = Command::new(std::env::current_exe().expect("test binary"))
            .arg("second_process_cannot_lock_the_same_store")
            .env("MATRIX_MCP_LOCK_CHILD", dir)
            .output()
            .expect("spawn lock probe");
        status.status.code().unwrap_or(12)
    }
}
