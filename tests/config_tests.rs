use matrix_mcp_server_r2::config::Config;
use serial_test::serial;

#[test]
#[serial]
fn config_validates_homeserver_url() {
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
    std::env::set_var("MATRIX_HOMESERVER_URL", "https://matrix.example.com");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "bad-user-id");

    let config = Config::from_env().unwrap();
    let result = config.validate();
    assert!(result.is_err(), "Expected validation to fail for bad user ID");

    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
}

#[test]
#[serial]
fn config_valid_settings_pass() {
    std::env::set_var("MATRIX_HOMESERVER_URL", "https://matrix.example.com");
    std::env::set_var("MATRIX_ACCESS_TOKEN", "test-token");
    std::env::set_var("MATRIX_USER_ID", "@bot:example.com");
    std::env::set_var("PORT", "3000");

    let config = Config::from_env().unwrap();
    let result = config.validate();
    assert!(result.is_ok(), "Expected validation to pass: {:?}", result.err());
    assert_eq!(config.port, 3000);
    assert_eq!(config.matrix_user_id, "@bot:example.com");

    std::env::remove_var("MATRIX_HOMESERVER_URL");
    std::env::remove_var("MATRIX_ACCESS_TOKEN");
    std::env::remove_var("MATRIX_USER_ID");
    std::env::remove_var("PORT");
}
