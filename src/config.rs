use crate::error::{AppError, Result};
use std::env;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    pub matrix_homeserver_url: String,
    pub matrix_access_token: String,
    pub matrix_user_id: String,
    pub matrix_domain: Option<String>,
    pub matrix_client_id: Option<String>,
    pub matrix_client_secret: Option<String>,

    pub port: u16,
    pub enable_oauth: bool,
    pub enable_token_exchange: bool,
    pub enable_https: bool,
    pub ssl_key_path: Option<String>,
    pub ssl_cert_path: Option<String>,
    pub cors_allowed_origins: Option<String>,

    pub idp_issuer_url: Option<String>,
    pub idp_authorization_url: Option<String>,
    pub idp_token_url: Option<String>,
    pub idp_registration_url: Option<String>,
    pub idp_revocation_url: Option<String>,
    pub oauth_callback_url: Option<String>,
    pub mcp_server_url: Option<String>,

    pub e2ee_enabled: bool,
    pub crypto_store_path: Option<String>,
    /// Recovery-key file path. Defaults to `$CRYPTO_STORE_PATH/recovery-key`
    /// when a store path is set. The file contents are never read or logged here.
    pub recovery_key_file: Option<String>,
    /// Optional path to a mode-0600 file holding the account password.
    /// Used later for UIAA on `/keys/device_signing/upload`. The password is
    /// never read or logged during configuration.
    pub uiaa_password_file: Option<String>,
    pub debug: bool,

    /// When true, start the MCP server without connecting to Matrix.
    /// Tools will return errors, but the MCP protocol layer is fully
    /// functional. Useful for MCP Inspector validation.
    pub skip_matrix_init: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let skip_matrix_init = env::var("SKIP_MATRIX_INIT")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let matrix_homeserver_url = env::var("MATRIX_HOMESERVER_URL").unwrap_or_else(|_| {
            if skip_matrix_init {
                "http://localhost:8008".into()
            } else {
                String::new()
            }
        });
        if matrix_homeserver_url.is_empty() {
            return Err(AppError::Config(
                "MATRIX_HOMESERVER_URL environment variable is required".into(),
            ));
        }

        let matrix_access_token = env::var("MATRIX_ACCESS_TOKEN").unwrap_or_else(|_| {
            if skip_matrix_init {
                "skip".into()
            } else {
                String::new()
            }
        });
        if matrix_access_token.is_empty() {
            return Err(AppError::Config(
                "MATRIX_ACCESS_TOKEN environment variable is required".into(),
            ));
        }

        let matrix_user_id = env::var("MATRIX_USER_ID").unwrap_or_else(|_| {
            if skip_matrix_init {
                "@mcp:localhost".into()
            } else {
                String::new()
            }
        });
        if matrix_user_id.is_empty() {
            return Err(AppError::Config(
                "MATRIX_USER_ID environment variable is required".into(),
            ));
        }

        let port = env::var("PORT")
            .or_else(|_| env::var("MCP_PORT"))
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let crypto_store_path = nonempty_env("CRYPTO_STORE_PATH");
        let recovery_key_file = recovery_key_file_from_env(crypto_store_path.as_deref());

        Ok(Config {
            matrix_homeserver_url,
            matrix_access_token,
            matrix_user_id,
            matrix_domain: env::var("MATRIX_DOMAIN").ok(),
            matrix_client_id: env::var("MATRIX_CLIENT_ID").ok(),
            matrix_client_secret: env::var("MATRIX_CLIENT_SECRET").ok(),
            port,
            enable_oauth: env::var("ENABLE_OAUTH")
                .map(|v| v == "true")
                .unwrap_or(false),
            enable_token_exchange: env::var("ENABLE_TOKEN_EXCHANGE")
                .map(|v| v == "true")
                .unwrap_or(false),
            enable_https: env::var("ENABLE_HTTPS")
                .map(|v| v == "true")
                .unwrap_or(false),
            ssl_key_path: env::var("SSL_KEY_PATH").ok(),
            ssl_cert_path: env::var("SSL_CERT_PATH").ok(),
            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS").ok(),
            idp_issuer_url: env::var("IDP_ISSUER_URL").ok(),
            idp_authorization_url: env::var("IDP_AUTHORIZATION_URL").ok(),
            idp_token_url: env::var("IDP_TOKEN_URL").ok(),
            idp_registration_url: env::var("IDP_REGISTRATION_URL").ok(),
            idp_revocation_url: env::var("IDP_REVOCATION_URL").ok(),
            oauth_callback_url: env::var("OAUTH_CALLBACK_URL").ok(),
            mcp_server_url: env::var("MCP_SERVER_URL").ok(),
            e2ee_enabled: env::var("E2EE_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            crypto_store_path,
            recovery_key_file,
            uiaa_password_file: nonempty_env("MATRIX_UIAA_PASSWORD_FILE"),
            debug: env::var("DEBUG")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            skip_matrix_init: env::var("SKIP_MATRIX_INIT")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        })
    }

    pub fn validate(&self) -> Result<()> {
        if !self.matrix_homeserver_url.starts_with("http://")
            && !self.matrix_homeserver_url.starts_with("https://")
        {
            return Err(AppError::Config(
                "MATRIX_HOMESERVER_URL must start with http:// or https://".into(),
            ));
        }

        if !self.matrix_user_id.starts_with('@') || !self.matrix_user_id.contains(':') {
            return Err(AppError::Config(
                "MATRIX_USER_ID must be in format @user:domain.com".into(),
            ));
        }

        if self.e2ee_enabled && self.crypto_store_path.is_none() {
            return Err(AppError::Config(
                "CRYPTO_STORE_PATH is required when E2EE_ENABLED=true".into(),
            ));
        }

        // The store, device id, and sync loop that use this flag are compiled
        // only with `--features e2ee`. Refuse a config that asks for E2EE
        // from a binary that cannot open the store.
        #[cfg(not(feature = "e2ee"))]
        if self.e2ee_enabled {
            return Err(AppError::Config(
                "E2EE_ENABLED=true requires a binary built with --features e2ee".into(),
            ));
        }

        if let Some(path) = &self.uiaa_password_file {
            reject_loose_uiaa_file(path)?;
        }

        Ok(())
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn recovery_key_file_from_env(crypto_store_path: Option<&str>) -> Option<String> {
    if let Some(path) = nonempty_env("MATRIX_RECOVERY_KEY_FILE") {
        return Some(path);
    }
    crypto_store_path.map(|dir| {
        std::path::Path::new(dir)
            .join("recovery-key")
            .to_string_lossy()
            .into_owned()
    })
}

/// If the UIAA password file exists, it must be mode 0600. A missing file is
/// left for bootstrap to report. The file contents are not read.
fn reject_loose_uiaa_file(path: &str) -> Result<()> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(AppError::Config(format!(
                "MATRIX_UIAA_PASSWORD_FILE could not be inspected: {error}"
            )));
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(AppError::Config(format!(
                "MATRIX_UIAA_PASSWORD_FILE must be mode 0600 (found {mode:o})"
            )));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
    }
    Ok(())
}
