use matrix_mcp_server_r2::config::Config;
use serial_test::serial;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

fn clear_e2ee_env() {
    for name in [
        "E2EE_ENABLED",
        "CRYPTO_STORE_PATH",
        "MATRIX_RECOVERY_KEY_FILE",
        "MATRIX_UIAA_PASSWORD_FILE",
    ] {
        std::env::remove_var(name);
    }
}

#[test]
#[serial]
fn config_validates_homeserver_url() {
    clear_e2ee_env();
    std::env::set_var("MATRIX_HOMESERVER_URL", "not-a-url");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "@bot:example.com");

    let config = Config::from_env().unwrap();
    let result = config.validate();
    assert!(result.is_err(), "Expected validation to fail for bad URL");

    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
}

#[test]
#[serial]
fn config_validates_user_id_format() {
    clear_e2ee_env();
    std::env::set_var("MATRIX_HOMESERVER_URL", "https://matrix.example.com");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "bad-user-id");

    let config = Config::from_env().unwrap();
    let result = config.validate();
    assert!(
        result.is_err(),
        "Expected validation to fail for bad user ID"
    );

    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
}

#[test]
#[serial]
fn config_valid_settings_pass() {
    clear_e2ee_env();
    std::env::set_var("MATRIX_HOMESERVER_URL", "https://matrix.example.com");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "@bot:example.com");
    std::env::set_var("PORT", "3000");

    let config = Config::from_env().unwrap();
    let result = config.validate();
    assert!(
        result.is_ok(),
        "Expected validation to pass: {:?}",
        result.err()
    );
    assert_eq!(config.port, 3000);
    assert_eq!(config.matrix_user_id, "@bot:example.com");

    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
    std::env::remove_var("PORT");
    clear_e2ee_env();
}

fn base_env() {
    clear_e2ee_env();
    std::env::set_var("MATRIX_HOMESERVER_URL", "https://matrix.example.com");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "@bot:example.com");
}

fn clear_base_env() {
    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
    clear_e2ee_env();
}

#[test]
#[serial]
fn e2ee_requires_crypto_store_path() {
    base_env();
    std::env::set_var("E2EE_ENABLED", "true");

    let config = Config::from_env().unwrap();
    let err = config.validate().expect_err("missing store path");
    assert!(
        err.to_string().contains("CRYPTO_STORE_PATH is required"),
        "{err}"
    );

    clear_base_env();
}

#[test]
#[serial]
fn recovery_key_file_defaults_beside_the_store() {
    base_env();
    std::env::set_var("E2EE_ENABLED", "true");
    std::env::set_var("CRYPTO_STORE_PATH", "/tmp/agent-store");

    let config = Config::from_env().unwrap();
    assert_eq!(
        config.recovery_key_file.as_deref(),
        Some("/tmp/agent-store/recovery-key")
    );
    assert!(config.uiaa_password_file.is_none());

    #[cfg(feature = "e2ee")]
    config
        .validate()
        .expect("store path satisfies E2EE_ENABLED");
    #[cfg(not(feature = "e2ee"))]
    {
        let err = config.validate().expect_err("feature missing");
        assert!(err.to_string().contains("--features e2ee"), "{err}");
    }

    clear_base_env();
}

#[test]
#[serial]
fn recovery_key_file_env_overrides_the_default() {
    base_env();
    std::env::set_var("CRYPTO_STORE_PATH", "/tmp/agent-store");
    std::env::set_var("MATRIX_RECOVERY_KEY_FILE", "/run/secrets/recovery-key");
    std::env::set_var("MATRIX_UIAA_PASSWORD_FILE", "/tmp/does-not-exist-uiaa");

    let config = Config::from_env().unwrap();
    assert_eq!(
        config.recovery_key_file.as_deref(),
        Some("/run/secrets/recovery-key")
    );
    assert_eq!(
        config.uiaa_password_file.as_deref(),
        Some("/tmp/does-not-exist-uiaa")
    );
    // Missing UIAA file is allowed. Validation must not read a password.
    config.validate().expect("unset E2EE_ENABLED stays valid");

    clear_base_env();
}

#[test]
#[serial]
fn uiaa_password_file_must_be_mode_0600_when_present() {
    base_env();
    let path = std::env::temp_dir().join(format!("matrix-mcp-uiaa-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&path)
        .unwrap();
    drop(file);
    std::env::set_var("MATRIX_UIAA_PASSWORD_FILE", &path);

    let config = Config::from_env().unwrap();
    let err = config.validate().expect_err("loose mode");
    assert!(err.to_string().contains("0600"), "{err}");
    assert!(!err.to_string().contains("secret-password"));

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    // Write a password and confirm validate still does not echo it.
    std::fs::write(&path, "secret-password").unwrap();
    let config = Config::from_env().unwrap();
    config.validate().expect("0600 file is accepted");
    assert!(!format!("{config:?}").contains("secret-password"));

    let _ = std::fs::remove_file(&path);
    clear_base_env();
}
