//! Integration tests using wiremock to mock a Matrix homeserver.
//!
//! These tests exercise the tool methods on MatrixMcpServer directly,
//! verifying that matrix-sdk calls are translated into the correct
//! MCP tool results.

use matrix_mcp_server_r2::config::Config;
use matrix_mcp_server_r2::mcp::server::*;
use matrix_sdk::{
    authentication::matrix::{MatrixSession, MatrixSessionTokens},
    config::SyncSettings,
    Client, SessionMeta,
};
use rmcp::handler::server::wrapper::Parameters;
use ruma::{OwnedDeviceId, OwnedUserId};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_USER: &str = "@bot:localhost";
const TEST_TOKEN: &str = "test_access_token";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_config(homeserver_url: &str) -> Arc<Config> {
    Arc::new(Config {
        matrix_homeserver_url: homeserver_url.to_string(),
        matrix_access_token: TEST_TOKEN.to_string(),
        matrix_user_id: TEST_USER.to_string(),
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
        debug: false,
        skip_matrix_init: false,
    })
}

/// Minimal /sync response with one joined room.
fn sync_response_one_room() -> serde_json::Value {
    json!({
        "next_batch": "s1_batch",
        "rooms": {
            "join": {
                "!testroom:localhost": {
                    "timeline": {
                        "events": [
                            {
                                "type": "m.room.message",
                                "event_id": "$msg1",
                                "sender": "@alice:localhost",
                                "origin_server_ts": 1700000000000u64,
                                "content": {
                                    "msgtype": "m.text",
                                    "body": "Hello from Alice"
                                }
                            },
                            {
                                "type": "m.room.message",
                                "event_id": "$msg2",
                                "sender": "@bob:localhost",
                                "origin_server_ts": 1700000060000u64,
                                "content": {
                                    "msgtype": "m.text",
                                    "body": "Hi Alice!"
                                }
                            }
                        ],
                        "prev_batch": "p1",
                        "limited": false
                    },
                    "state": {
                        "events": [
                            {
                                "type": "m.room.create",
                                "state_key": "",
                                "event_id": "$create",
                                "sender": "@alice:localhost",
                                "origin_server_ts": 1699999000000u64,
                                "content": { "creator": "@alice:localhost", "room_version": "10" }
                            },
                            {
                                "type": "m.room.name",
                                "state_key": "",
                                "event_id": "$name",
                                "sender": "@alice:localhost",
                                "origin_server_ts": 1699999001000u64,
                                "content": { "name": "Test Room" }
                            },
                            {
                                "type": "m.room.topic",
                                "state_key": "",
                                "event_id": "$topic",
                                "sender": "@alice:localhost",
                                "origin_server_ts": 1699999002000u64,
                                "content": { "topic": "A test room for integration tests" }
                            },
                            {
                                "type": "m.room.member",
                                "state_key": "@bot:localhost",
                                "event_id": "$join_bot",
                                "sender": "@bot:localhost",
                                "origin_server_ts": 1699999003000u64,
                                "content": { "membership": "join", "displayname": "MCP Bot" }
                            },
                            {
                                "type": "m.room.member",
                                "state_key": "@alice:localhost",
                                "event_id": "$join_alice",
                                "sender": "@alice:localhost",
                                "origin_server_ts": 1699999004000u64,
                                "content": { "membership": "join", "displayname": "Alice" }
                            },
                            {
                                "type": "m.room.member",
                                "state_key": "@bob:localhost",
                                "event_id": "$join_bob",
                                "sender": "@bob:localhost",
                                "origin_server_ts": 1699999005000u64,
                                "content": { "membership": "join", "displayname": "Bob" }
                            }
                        ]
                    },
                    "account_data": { "events": [] },
                    "ephemeral": { "events": [] },
                    "unread_notifications": {
                        "notification_count": 2,
                        "highlight_count": 1
                    }
                }
            },
            "invite": {},
            "leave": {}
        },
        "presence": { "events": [] },
        "account_data": { "events": [] },
        "to_device": { "events": [] },
        "device_lists": { "changed": [], "left": [] }
    })
}

/// Empty /sync response (no rooms).
fn sync_response_empty() -> serde_json::Value {
    json!({
        "next_batch": "s_empty",
        "rooms": { "join": {}, "invite": {}, "leave": {} },
        "presence": { "events": [] },
        "account_data": { "events": [] },
        "to_device": { "events": [] },
        "device_lists": { "changed": [], "left": [] }
    })
}

/// Mount all the standard Matrix API mocks needed before sync_once works.
async fn mount_base_mocks(mock: &MockServer, sync_body: serde_json::Value) {
    // Versions endpoint -- matrix-sdk may probe this
    Mock::given(method("GET"))
        .and(path("/_matrix/client/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": ["v1.1", "v1.2", "v1.3"],
            "unstable_features": {}
        })))
        .mount(mock)
        .await;

    // Sync endpoint
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sync_body))
        .mount(mock)
        .await;

    // Filter endpoint (matrix-sdk may try to upload a filter)
    Mock::given(method("POST"))
        .and(path_regex::PathRegexMatcher::new(
            r"/_matrix/client/v3/user/.*/filter",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "filter_id": "0" })))
        .mount(mock)
        .await;

    // Capabilities (sometimes probed during build)
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/capabilities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "capabilities": {} })))
        .mount(mock)
        .await;

    // Pushrules (probed during sync setup)
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/pushrules/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "global": {} })))
        .mount(mock)
        .await;
}

/// Create a matrix-sdk Client connected to the given wiremock server,
/// with session restored and one sync completed.
async fn create_test_client(mock_server: &MockServer) -> Client {
    let url = mock_server.uri();

    let client = Client::builder()
        .homeserver_url(&url)
        .build()
        .await
        .expect("Failed to build test client");

    let session = MatrixSession {
        meta: SessionMeta {
            user_id: OwnedUserId::try_from(TEST_USER).unwrap(),
            device_id: OwnedDeviceId::from("TEST_DEVICE"),
        },
        tokens: MatrixSessionTokens {
            access_token: TEST_TOKEN.to_string(),
            refresh_token: None,
        },
    };
    client
        .matrix_auth()
        .restore_session(session)
        .await
        .expect("Failed to restore test session");

    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("Test sync_once failed");

    client
}

fn build_server(client: Client, homeserver_url: &str) -> MatrixMcpServer {
    MatrixMcpServer::new(client, test_config(homeserver_url))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_joined_rooms_with_one_room() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let result = server.list_joined_rooms().await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("Test Room"), "Expected room name in output: {}", text);
    assert!(text.contains("!testroom:localhost"), "Expected room ID in output: {}", text);
}

#[tokio::test]
async fn test_list_joined_rooms_empty() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_empty()).await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let result = server.list_joined_rooms().await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("No joined rooms"), "Expected empty message: {}", text);
}

#[tokio::test]
async fn test_get_room_info() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let input = Parameters(RoomIdInput {
        room_id: "!testroom:localhost".into(),
    });
    let result = server.get_room_info(input).await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("Test Room"), "Expected room name: {}", text);
    assert!(text.contains("A test room for integration tests"), "Expected topic: {}", text);
}

#[tokio::test]
async fn test_get_room_info_not_found() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let input = Parameters(RoomIdInput {
        room_id: "!nonexistent:localhost".into(),
    });
    let result = server.get_room_info(input).await.expect("tool call failed");
    assert!(
        result.is_error == Some(true),
        "Expected is_error=true for missing room"
    );
}

#[tokio::test]
async fn test_get_room_members() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;
    // get_room_members calls GET /rooms/{roomId}/members
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/rooms/!testroom:localhost/members"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "chunk": [
                {
                    "type": "m.room.member",
                    "state_key": "@alice:localhost",
                    "sender": "@alice:localhost",
                    "content": { "membership": "join", "displayname": "Alice" },
                    "event_id": "$m1",
                    "origin_server_ts": 1699999004000u64,
                    "room_id": "!testroom:localhost"
                },
                {
                    "type": "m.room.member",
                    "state_key": "@bob:localhost",
                    "sender": "@bob:localhost",
                    "content": { "membership": "join", "displayname": "Bob" },
                    "event_id": "$m2",
                    "origin_server_ts": 1699999005000u64,
                    "room_id": "!testroom:localhost"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let input = Parameters(RoomIdInput {
        room_id: "!testroom:localhost".into(),
    });
    let result = server.get_room_members(input).await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("Alice"), "Expected Alice in members: {}", text);
    assert!(text.contains("Bob"), "Expected Bob in members: {}", text);
}

#[tokio::test]
async fn test_get_my_profile() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/profile/@bot:localhost/displayname"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "displayname": "MCP Bot" })),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/profile/@bot:localhost/avatar_url"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({})),
        )
        .mount(&mock)
        .await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let result = server.get_my_profile().await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("MCP Bot") || text.contains("@bot:localhost"), "Expected profile info: {}", text);
    assert!(text.contains("Joined Rooms"), "Expected room count: {}", text);
}

#[tokio::test]
async fn test_get_notification_counts() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let input = Parameters(GetNotificationCountsInput { room_filter: None });
    let result = server
        .get_notification_counts(input)
        .await
        .expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(
        text.contains("Notification Summary") || text.contains("Unread"),
        "Expected notification info: {}",
        text
    );
}

#[tokio::test]
async fn test_send_message() {
    let mock = MockServer::start().await;
    mount_base_mocks(&mock, sync_response_one_room()).await;
    // Mock the send-event endpoint
    Mock::given(method("PUT"))
        .and(path_regex::PathRegexMatcher::new(
            r"/_matrix/client/v3/rooms/.*/send/m.room.message/.*",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "event_id": "$sent1" })),
        )
        .mount(&mock)
        .await;

    let client = create_test_client(&mock).await;
    let server = build_server(client, &mock.uri());

    let input = Parameters(SendMessageInput {
        room_id: "!testroom:localhost".into(),
        message: "Hello world".into(),
        message_type: None,
        reply_to_event_id: None,
    });
    let result = server.send_message(input).await.expect("tool call failed");
    let text = format!("{:?}", result);
    assert!(text.contains("sent successfully"), "Expected success message: {}", text);
    assert!(text.contains("$sent1"), "Expected event ID: {}", text);
}

// Helper: wiremock path_regex matcher (not in wiremock::matchers by default)
mod path_regex {
    use wiremock::{Match, Request};

    pub struct PathRegexMatcher {
        re: regex::Regex,
    }

    impl PathRegexMatcher {
        pub fn new(pattern: &str) -> Self {
            Self {
                re: regex::Regex::new(pattern).unwrap(),
            }
        }
    }

    impl Match for PathRegexMatcher {
        fn matches(&self, request: &Request) -> bool {
            self.re.is_match(request.url.path())
        }
    }
}
