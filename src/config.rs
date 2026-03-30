use crate::error::{AppError, Result};
use std::env;

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
            if skip_matrix_init { "skip".into() } else { String::new() }
        });
        if matrix_access_token.is_empty() {
            return Err(AppError::Config(
                "MATRIX_ACCESS_TOKEN environment variable is required".into(),
            ));
        }

        let matrix_user_id = env::var("MATRIX_USER_ID").unwrap_or_else(|_| {
            if skip_matrix_init { "@mcp:localhost".into() } else { String::new() }
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
            crypto_store_path: env::var("CRYPTO_STORE_PATH").ok(),
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

        Ok(())
    }
}
