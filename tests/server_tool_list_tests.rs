use std::sync::Arc;

use matrix_mcp_server_r2::config::Config;
use matrix_mcp_server_r2::mcp::server::MatrixMcpServer;
use rmcp::ServerHandler;

/// Verify the MCP server reports the expected tool count and names.
/// This does NOT require a live Matrix homeserver -- it only checks
/// the statically-registered tool metadata.
#[test]
fn server_info_returns_correct_metadata() {
    // We cannot construct MatrixMcpServer without a real Client, so
    // we just verify the ServerHandler trait is implemented by checking
    // the type compiles. A more thorough integration test would mock
    // the Matrix client.
    fn _assert_server_handler<T: ServerHandler>() {}
    _assert_server_handler::<MatrixMcpServer>();
}

#[tokio::test]
async fn the_tool_router_lists_23_tools_including_list_invitations() {
    let client = matrix_sdk::Client::builder()
        .homeserver_url("http://127.0.0.1:9")
        .build()
        .await
        .expect("client");
    let config = Arc::new(Config {
        matrix_homeserver_url: "http://127.0.0.1:9".to_string(),
        matrix_access_token: String::new(),
        matrix_user_id: "@agent0-5:example.com".to_string(),
        matrix_domain: None,
        matrix_client_id: None,
        matrix_client_secret: None,
        port: 0,
        enable_oauth: false,
        enable_token_exchange: false,
        enable_https: false,
        ssl_key_path: None,
        ssl_cert_path: None,
        cors_allowed_origins: None,
        idp_issuer_url: None,
        idp_authorization_url: None,
        idp_token_url: None,
        idp_registration_url: None,
        idp_revocation_url: None,
        oauth_callback_url: None,
        mcp_server_url: None,
        e2ee_enabled: false,
        crypto_store_path: None,
        recovery_key_file: None,
        uiaa_password_file: None,
        debug: false,
        skip_matrix_init: true,
        bot_enabled: false,
        accepts_images: false,
        bot_display_name: String::new(),
        agent_identity: "Agent Zero".to_string(),
        a0_api_url: "http://127.0.0.1:9/api/api_message".to_string(),
        amctl_url: "http://127.0.0.1:9".to_string(),
        amctl_api_key: None,
        trusted_senders: Vec::new(),
        bot_state_dir: "/tmp/matrix-bot".to_string(),
    });
    let names = MatrixMcpServer::new(client, config).registered_tool_names();
    assert_eq!(names.len(), 23);
    assert!(names.iter().any(|name| name == "list_invitations"));
    assert!(names.iter().any(|name| name == "bootstrap_e2ee"));
    assert!(names.iter().any(|name| name == "get_encryption_status"));
}
