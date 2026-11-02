use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[allow(dead_code)]
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

    /// The crypto store path is missing, not a directory, or already locked
    /// by another process.
    #[error(transparent)]
    KeyStoreUnavailable(#[from] KeyStoreUnavailableError),

    /// A named recovery secret could not be resolved. The message never
    /// contains the secret.
    #[cfg(any(test, feature = "e2ee"))]
    #[error(transparent)]
    SecretResolution(#[from] SecretResolutionError),

    /// The homeserver already has a cross-signing identity and no recovery
    /// secret was available to unlock it.
    #[cfg(feature = "e2ee")]
    #[error(transparent)]
    IdentityExists(#[from] IdentityExistsError),

    /// Creating a cross-signing identity needs UIAA and the password file
    /// is missing or unreadable.
    #[cfg(feature = "e2ee")]
    #[error(transparent)]
    UiaaRequired(#[from] UiaaRequiredError),

    /// The caller asked for a trust policy other than `cross-signed`.
    #[cfg(any(test, feature = "e2ee"))]
    #[error(transparent)]
    TrustPolicyNotSatisfied(#[from] TrustPolicyNotSatisfiedError),
}

/// Second open of `CRYPTO_STORE_PATH`, or a store that cannot be used safely.
///
/// The message describes the failure. It must not contain recovery keys,
/// access tokens, or account passwords.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct KeyStoreUnavailableError(String);

impl KeyStoreUnavailableError {
    /// Build an error whose text is safe to log and return to callers.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// `backupPassphraseRef` was passed with no secret-manager backend, or a
/// named env var or recovery-key file could not be read.
#[cfg(any(test, feature = "e2ee"))]
#[derive(Debug, Error)]
#[error("SecretResolutionError: {0}")]
pub struct SecretResolutionError(String);

#[cfg(any(test, feature = "e2ee"))]
impl SecretResolutionError {
    /// Build an error whose text is safe to log and return to callers.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// The account already has a cross-signing identity and nothing unlocked it.
#[cfg(feature = "e2ee")]
#[derive(Debug, Error)]
#[error("IdentityExistsError: {0}")]
pub struct IdentityExistsError(String);

#[cfg(feature = "e2ee")]
impl IdentityExistsError {
    /// Build an error whose text is safe to log and return to callers.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// UIAA is required and `MATRIX_UIAA_PASSWORD_FILE` cannot supply a password.
#[cfg(feature = "e2ee")]
#[derive(Debug, Error)]
#[error("UiaaRequiredError: {0}")]
pub struct UiaaRequiredError(String);

#[cfg(feature = "e2ee")]
impl UiaaRequiredError {
    /// Build an error whose text is safe to log and return to callers.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// `trustPolicy` was not `cross-signed`.
#[cfg(any(test, feature = "e2ee"))]
#[derive(Debug, Error)]
#[error("TrustPolicyNotSatisfiedError: {0}")]
pub struct TrustPolicyNotSatisfiedError(String);

#[cfg(any(test, feature = "e2ee"))]
impl TrustPolicyNotSatisfiedError {
    /// Build an error whose text is safe to log and return to callers.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
