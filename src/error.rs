use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Matrix client error: {0}")]
    MatrixClient(String),

    #[error("Matrix SDK error: {0}")]
    MatrixSdk(#[from] matrix_sdk::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] axum::http::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MCP protocol error: {0}")]
    Mcp(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Unknown error: {0}")]
    Unknown(String),
}
