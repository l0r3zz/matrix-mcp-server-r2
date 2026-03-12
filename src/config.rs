use crate::error::{AppError, Result};
use std::env;

/// Server configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Matrix homeserver URL (e.g., https://matrix.example.com)
    pub matrix_homeserver_url: String,
    
    /// Matrix access token for authentication
    pub matrix_access_token: String,
    
    /// Matrix user ID (e.g., @user:example.com)
    pub matrix_user_id: String,
    
    /// Port to run the MCP server on
    pub mcp_port: u16,
    
    /// Optional API key for MCP authentication
    pub mcp_api_key: Option<String>,
    
    /// Enable debug logging
    pub debug: bool,
    
    /// Path to store crypto store (for E2EE)
    pub crypto_store_path: Option<String>,
    
    /// Enable E2EE features
    pub e2ee_enabled: bool,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Load .env file if present (ignore errors)
        let _ = dotenvy::dotenv();
        
        let matrix_homeserver_url = env::var("MATRIX_HOMESERVER_URL")
            .map_err(|_| AppError::Config(
                "MATRIX_HOMESERVER_URL environment variable is required".to_string()
            ))?;
        
        let matrix_access_token = env::var("MATRIX_ACCESS_TOKEN")
            .map_err(|_| AppError::Config(
                "MATRIX_ACCESS_TOKEN environment variable is required".to_string()
            ))?;
        
        let matrix_user_id = env::var("MATRIX_USER_ID")
            .map_err(|_| AppError::Config(
                "MATRIX_USER_ID environment variable is required".to_string()
            ))?;
        
        let mcp_port = env::var("MCP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        
        let mcp_api_key = env::var("MCP_API_KEY").ok();
        
        let debug = env::var("DEBUG")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        
        let crypto_store_path = env::var("CRYPTO_STORE_PATH").ok();
        
        let e2ee_enabled = env::var("E2EE_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        
        Ok(Config {
            matrix_homeserver_url,
            matrix_access_token,
            matrix_user_id,
            mcp_port,
            mcp_api_key,
            debug,
            crypto_store_path,
            e2ee_enabled,
        })
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate homeserver URL
        if !self.matrix_homeserver_url.starts_with("http://") 
            && !self.matrix_homeserver_url.starts_with("https://") {
            return Err(AppError::Config(
                "MATRIX_HOMESERVER_URL must start with http:// or https://".to_string()
            ));
        }
        
        // Validate user ID format (@user:domain.com)
        if !self.matrix_user_id.starts_with('@') || !self.matrix_user_id.contains(':') {
            return Err(AppError::Config(
                "MATRIX_USER_ID must be in format @user:domain.com".to_string()
            ));
        }
        
        Ok(())
    }
}
