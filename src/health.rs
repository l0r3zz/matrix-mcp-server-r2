//! `GET /health` body. The flags follow the process config, not a room.

use serde_json::{json, Value};

/// `e2ee` is true only when this binary has the feature, the operator turned
/// it on, and startup did not skip Matrix. `crossSigned` stays false unless
/// that is true and the local device is cross-signed.
pub fn health_flags(
    e2ee_enabled: bool,
    skip_matrix_init: bool,
    device_cross_signed: bool,
) -> (bool, bool) {
    let e2ee = cfg!(feature = "e2ee") && e2ee_enabled && !skip_matrix_init;
    (e2ee, e2ee && device_cross_signed)
}

/// JSON body for `GET /health`. Both crypto flags are always present.
pub fn health_response(user_id: &str, version: &str, e2ee: bool, cross_signed: bool) -> Value {
    json!({
        "status": "healthy",
        "server": "matrix-mcp-server-r2",
        "version": version,
        "user_id": user_id,
        "e2ee": e2ee,
        "crossSigned": cross_signed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_matrix_init_forces_both_flags_false() {
        let (e2ee, cross_signed) = health_flags(true, true, true);
        assert!(!e2ee);
        assert!(!cross_signed);
        let body = health_response("@bot:localhost", "0.1.1", e2ee, cross_signed);
        assert_eq!(body["e2ee"], false);
        assert_eq!(body["crossSigned"], false);
        assert_eq!(body["status"], "healthy");
    }
}
