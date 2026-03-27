/// Per-request authentication context extracted from HTTP headers.
///
/// Mirrors the TS `getMatrixContext()` and `getAccessToken()` helpers.
/// Falls back to server-wide config when headers are absent.
pub struct MatrixAuthContext {
    pub user_id: String,
    pub homeserver_url: String,
    pub access_token: String,
}

impl MatrixAuthContext {
    /// Build auth context from explicit values (server-wide config).
    pub fn from_config(user_id: &str, homeserver_url: &str, access_token: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            homeserver_url: homeserver_url.to_string(),
            access_token: access_token.to_string(),
        }
    }
}
