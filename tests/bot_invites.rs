//! Invite reconciliation against a wiremock homeserver.

use std::sync::Arc;
use std::time::{Duration, Instant};

use matrix_mcp_server_r2::bot::invites::{
    reconcile_pass, runtime_for, spawn_reconciler, InviteRegistry, RetryPolicy,
};
use matrix_mcp_server_r2::bot::Session;
use matrix_mcp_server_r2::config::Config;
use matrix_mcp_server_r2::mcp::server::MatrixMcpServer;
use matrix_sdk::{
    authentication::matrix::MatrixSession, config::SyncSettings, store::RoomLoadSettings, Client,
    SessionMeta, SessionTokens,
};
use ruma::{OwnedDeviceId, OwnedUserId};
use serde_json::json;
use serial_test::serial;
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const SESSION_USER: &str = "@agent0-5:example.com";
const TRUSTED: &str = "@agent0-5:example.com";
const OTHER: &str = "@other:example.com";
const ROOM: &str = "!ops:example.com";

fn session(amctl_url: String, fallback: Vec<String>) -> Session {
    let dir = std::env::temp_dir().join(format!(
        "am-invite-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    Session {
        user_id: SESSION_USER.to_string(),
        display_name: "Agent".to_string(),
        identity: "Agent Zero".to_string(),
        state_dir: dir,
        a0_api_url: "http://localhost:80/api/api_message".to_string(),
        amctl_url,
        amctl_api_key: Some("lab-test-key".to_string()),
        trusted_senders: fallback,
        accepts_images: false,
    }
}

fn invite_sync(inviter: &str) -> serde_json::Value {
    json!({
        "next_batch": "s1",
        "rooms": {
            "invite": {
                ROOM: {
                    "invite_state": {
                        "events": [
                            member_event(inviter, inviter, "join"),
                            member_event(SESSION_USER, inviter, "invite"),
                            state_event("m.room.create", inviter, json!({"creator": inviter, "room_version": "10"})),
                            state_event("m.room.name", inviter, json!({"name": "Ops"})),
                            state_event("m.room.join_rules", inviter, json!({"join_rule": "invite"}))
                        ]
                    }
                }
            }
        }
    })
}

fn member_event(state_key: &str, sender: &str, membership: &str) -> serde_json::Value {
    json!({
        "type": "m.room.member",
        "state_key": state_key,
        "sender": sender,
        "content": { "membership": membership }
    })
}

fn state_event(event_type: &str, sender: &str, content: serde_json::Value) -> serde_json::Value {
    json!({
        "type": event_type,
        "state_key": "",
        "sender": sender,
        "content": content
    })
}

fn empty_sync() -> serde_json::Value {
    json!({
        "next_batch": "s0",
        "rooms": { "invite": {} }
    })
}

async fn mount_homeserver(mock: &MockServer, sync_body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/_matrix/client/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": ["v1.11"],
            "unstable_features": {}
        })))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sync_body))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/capabilities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"capabilities": {}})))
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(PathRegex::new(r"/_matrix/client/v3/user/.*/filter"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"filter_id": "0"})))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/pushrules/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"global": {}})))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(PathRegex::new(
            r"/_matrix/client/v3/rooms/.*/state/m\.room\.encryption/?$",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errcode": "M_NOT_FOUND",
            "error": "Event not found."
        })))
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(PathRegex::new(r"/join/?$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"room_id": ROOM})))
        .mount(mock)
        .await;
    Mock::given(method("PUT"))
        .and(PathRegex::new(r"/send/m\.room\.message/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id": "$greet"})))
        .mount(mock)
        .await;
}

struct PathRegex {
    re: regex::Regex,
}

impl PathRegex {
    fn new(pattern: &str) -> Self {
        Self {
            re: regex::Regex::new(pattern).expect("path pattern"),
        }
    }
}

impl Match for PathRegex {
    fn matches(&self, request: &Request) -> bool {
        self.re.is_match(request.url.path())
    }
}

async fn synced_client(mock: &MockServer) -> Client {
    let client = Client::builder()
        .homeserver_url(mock.uri())
        .request_config(matrix_sdk::config::RequestConfig::new().disable_retry())
        .build()
        .await
        .expect("client");
    let matrix_session = MatrixSession {
        meta: SessionMeta {
            user_id: OwnedUserId::try_from(SESSION_USER).expect("user"),
            device_id: OwnedDeviceId::from("DEVICE"),
        },
        tokens: SessionTokens {
            access_token: "test_access_token".to_string(),
            refresh_token: None,
        },
    };
    client
        .matrix_auth()
        .restore_session(matrix_session, RoomLoadSettings::default())
        .await
        .expect("session");
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("sync");
    client
}

async fn count_path(mock: &MockServer, verb: &str, fragment: &str) -> usize {
    mock.received_requests()
        .await
        .expect("requests")
        .iter()
        .filter(|request| request.method.as_str() == verb && request.url.path().contains(fragment))
        .count()
}

async fn trusted_server(body: serde_json::Value, status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/trusted-senders"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
#[serial]
async fn a_trusted_invite_joins_once_and_greets_once() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    reconcile_pass(
        &client,
        &runtime,
        &InviteRegistry::new(),
        &RetryPolicy::default(),
    )
    .await;

    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    assert_eq!(
        count_path(&homeserver, "PUT", "/send/m.room.message/").await,
        1
    );
}

#[tokio::test]
#[serial]
async fn an_untrusted_invite_is_not_joined() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(OTHER)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    reconcile_pass(
        &client,
        &runtime,
        &InviteRegistry::new(),
        &RetryPolicy::default(),
    )
    .await;

    assert_eq!(count_path(&homeserver, "POST", "/join").await, 0);
}

#[tokio::test]
#[serial]
async fn no_invites_do_not_call_amctl() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, empty_sync()).await;
    let amctl = trusted_server(json!([]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    reconcile_pass(
        &client,
        &runtime,
        &InviteRegistry::new(),
        &RetryPolicy::default(),
    )
    .await;

    let calls = amctl.received_requests().await.expect("requests").len();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[serial]
async fn a_failed_lookup_with_an_empty_fallback_does_not_join() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    let amctl = trusted_server(json!({"error": "down"}), 500).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    reconcile_pass(
        &client,
        &runtime,
        &InviteRegistry::new(),
        &RetryPolicy::default(),
    )
    .await;

    assert_eq!(count_path(&homeserver, "POST", "/join").await, 0);
}

fn fast_policy() -> RetryPolicy {
    RetryPolicy {
        base: Duration::from_millis(10),
        cap: Duration::from_millis(50),
        max_attempts: 5,
    }
}

fn due_now() -> RetryPolicy {
    RetryPolicy {
        base: Duration::ZERO,
        cap: Duration::ZERO,
        max_attempts: 5,
    }
}

fn test_config(homeserver_url: &str) -> Arc<Config> {
    Arc::new(Config {
        matrix_homeserver_url: homeserver_url.to_string(),
        matrix_access_token: "test_access_token".to_string(),
        matrix_user_id: SESSION_USER.to_string(),
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
        skip_matrix_init: false,
        bot_enabled: false,
        accepts_images: false,
        bot_display_name: String::new(),
        agent_identity: "Agent Zero".to_string(),
        a0_api_url: "http://127.0.0.1:9/api/api_message".to_string(),
        amctl_url: "http://127.0.0.1:9".to_string(),
        amctl_api_key: None,
        trusted_senders: Vec::new(),
        bot_state_dir: "/tmp/matrix-bot".to_string(),
    })
}

fn room_invite(
    name: &str,
    alias: &str,
    topic: &str,
    inviter: &str,
    encrypted: bool,
) -> serde_json::Value {
    let mut events = vec![
        member_event(inviter, inviter, "join"),
        member_event(SESSION_USER, inviter, "invite"),
        state_event(
            "m.room.create",
            inviter,
            json!({"creator": inviter, "room_version": "10"}),
        ),
        state_event("m.room.name", inviter, json!({"name": name})),
        state_event("m.room.join_rules", inviter, json!({"join_rule": "invite"})),
        state_event("m.room.topic", inviter, json!({"topic": topic})),
        state_event("m.room.canonical_alias", inviter, json!({"alias": alias})),
    ];
    if encrypted {
        events.push(state_event(
            "m.room.encryption",
            inviter,
            json!({"algorithm": "m.megolm.v1.aes-sha2"}),
        ));
    }
    json!({ "invite_state": { "events": events } })
}

fn two_invite_sync() -> serde_json::Value {
    json!({
        "next_batch": "s2",
        "rooms": {
            "invite": {
                "!bbb:example.com": room_invite("Beta", "#beta:example.com", "Beta chat", OTHER, false),
                "!aaa:example.com": room_invite("Alpha", "#alpha:example.com", "Alpha chat", TRUSTED, true)
            }
        }
    })
}

fn joined_sync() -> serde_json::Value {
    json!({
        "next_batch": "s3",
        "rooms": {
            "join": {
                ROOM: {
                    "timeline": { "events": [], "prev_batch": "p", "limited": false },
                    "state": { "events": [] }
                }
            },
            "invite": {}
        }
    })
}

fn left_sync() -> serde_json::Value {
    json!({
        "next_batch": "s4",
        "rooms": {
            "leave": {
                ROOM: {
                    "timeline": {
                        "events": [member_event(SESSION_USER, OTHER, "leave")],
                        "prev_batch": "p",
                        "limited": false
                    },
                    "state": { "events": [] }
                }
            },
            "invite": {}
        }
    })
}

async fn mount_status(mock: &MockServer, status: u16, body: serde_json::Value, times: Option<u64>) {
    let template = ResponseTemplate::new(status).set_body_json(body);
    let mut mounted = Mock::given(method("POST"))
        .and(PathRegex::new(r"/join/?$"))
        .respond_with(template)
        .with_priority(1);
    if let Some(times) = times {
        mounted = mounted.up_to_n_times(times);
    }
    mounted.mount(mock).await;
}

async fn tool_json(server: &MatrixMcpServer) -> serde_json::Value {
    let result = server.list_invitations().await.expect("list");
    let text = result.content[0].as_text().expect("text").text.clone();
    serde_json::from_str(&text).expect("json")
}

async fn wait_for_joins(mock: &MockServer, expected: usize) {
    let started = Instant::now();
    loop {
        if count_path(mock, "POST", "/join").await >= expected {
            return;
        }
        if started.elapsed() > Duration::from_secs(3) {
            panic!("timed out waiting for {expected} joins");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[serial]
async fn list_invitations_reports_two_unknown_invites() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, two_invite_sync()).await;
    let client = synced_client(&homeserver).await;
    let server = MatrixMcpServer::new(client, test_config(&homeserver.uri()))
        .with_invites(Arc::new(InviteRegistry::new()));
    let result = server.list_invitations().await.expect("list");
    let text = result.content[0].as_text().expect("text").text.clone();
    assert_eq!(result.content.len(), 1);
    assert!(!text.contains("test_access_token"));
    let body: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(body["count"], 2);
    assert_eq!(body["invitations"][0]["roomId"], "!aaa:example.com");
    assert_eq!(body["invitations"][0]["name"], "Alpha");
    assert_eq!(
        body["invitations"][0]["canonicalAlias"],
        "#alpha:example.com"
    );
    assert_eq!(body["invitations"][0]["topic"], "Alpha chat");
    assert_eq!(body["invitations"][0]["inviter"], TRUSTED);
    assert_eq!(body["invitations"][0]["trusted"], "unknown");
    assert_eq!(body["invitations"][0]["attempts"], 0);
    assert_eq!(body["invitations"][1]["roomId"], "!bbb:example.com");
    assert_eq!(body["invitations"][1]["name"], "Beta");
    assert_eq!(
        body["invitations"][1]["canonicalAlias"],
        "#beta:example.com"
    );
    assert_eq!(body["invitations"][1]["topic"], "Beta chat");
    assert_eq!(body["invitations"][1]["inviter"], OTHER);
    let encryption = body["invitations"][0]["encryption"]
        .as_str()
        .expect("encryption");
    assert!(matches!(
        encryption,
        "encrypted" | "unencrypted" | "unknown"
    ));
}

#[tokio::test]
#[serial]
async fn a_502_then_a_200_joins_twice_and_greets_once() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(&homeserver, 502, json!({"error": "down"}), Some(1)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let handle = spawn_reconciler(
        client.clone(),
        runtime,
        Arc::new(InviteRegistry::new()),
        Arc::new(tokio::sync::Notify::new()),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 2);
    assert_eq!(
        count_path(&homeserver, "PUT", "/send/m.room.message/").await,
        1
    );
    handle.abort();
}

#[tokio::test]
#[serial]
async fn five_502_responses_stop_at_five_attempts() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(&homeserver, 502, json!({"error": "down"}), None).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    let server = MatrixMcpServer::new(client.clone(), test_config(&homeserver.uri()))
        .with_invites(Arc::clone(&registry));
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::clone(&registry),
        Arc::new(tokio::sync::Notify::new()),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 5).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 5);
    let body = tool_json(&server).await;
    assert_eq!(body["invitations"][0]["trusted"], "true");
    assert_eq!(body["invitations"][0]["attempts"], 5);
    assert_eq!(body["invitations"][0]["lastError"], "temporary 502");
    handle.abort();
}

#[tokio::test]
#[serial]
async fn a_403_is_permanent_after_one_join() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(
        &homeserver,
        403,
        json!({"errcode": "M_FORBIDDEN", "error": "nope"}),
        None,
    )
    .await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    let server = MatrixMcpServer::new(client.clone(), test_config(&homeserver.uri()))
        .with_invites(Arc::clone(&registry));
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::clone(&registry),
        Arc::new(tokio::sync::Notify::new()),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 1).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    let body = tool_json(&server).await;
    assert_eq!(body["invitations"][0]["attempts"], 1);
    assert_eq!(
        body["invitations"][0]["lastError"],
        "permanent 403 M_FORBIDDEN"
    );
    assert!(!body.to_string().contains("nope"));
    handle.abort();
}

#[tokio::test]
#[serial]
async fn a_not_invited_403_retries_while_the_invite_remains() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(
        &homeserver,
        403,
        json!({
            "errcode": "M_FORBIDDEN",
            "error": "You are not invited to this room."
        }),
        Some(1),
    )
    .await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    let server = MatrixMcpServer::new(client.clone(), test_config(&homeserver.uri()))
        .with_invites(Arc::clone(&registry));
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::clone(&registry),
        Arc::new(tokio::sync::Notify::new()),
        RetryPolicy {
            base: Duration::from_millis(250),
            cap: Duration::from_millis(250),
            max_attempts: 5,
        },
    );
    wait_for_joins(&homeserver, 1).await;
    let pending = tool_json(&server).await;
    assert_eq!(pending["invitations"][0]["attempts"], 1);
    assert_eq!(
        pending["invitations"][0]["lastError"],
        "temporary 403 M_FORBIDDEN"
    );
    assert!(!pending.to_string().contains("not invited"));
    wait_for_joins(&homeserver, 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        count_path(&homeserver, "PUT", "/send/m.room.message/").await,
        1
    );
    handle.abort();
}

#[tokio::test]
#[serial]
async fn a_404_stops_after_one_join() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(
        &homeserver,
        404,
        json!({"errcode": "M_NOT_FOUND", "error": "Room not found."}),
        None,
    )
    .await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    let server = MatrixMcpServer::new(client.clone(), test_config(&homeserver.uri()))
        .with_invites(Arc::clone(&registry));
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::clone(&registry),
        Arc::new(tokio::sync::Notify::new()),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    let body = tool_json(&server).await;
    assert_eq!(body["invitations"][0]["attempts"], 1);
    assert_eq!(
        body["invitations"][0]["lastError"],
        "permanent 404 M_NOT_FOUND"
    );
    handle.abort();
}

#[tokio::test]
#[serial]
async fn a_403_after_the_invite_is_gone_does_not_join_again() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    mount_status(
        &homeserver,
        403,
        json!({
            "errcode": "M_FORBIDDEN",
            "error": "You are not invited to this room."
        }),
        None,
    )
    .await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    reconcile_pass(&client, &runtime, &registry, &due_now()).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(left_sync()))
        .with_priority(1)
        .mount(&homeserver)
        .await;
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("sync");
    reconcile_pass(&client, &runtime, &registry, &due_now()).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    assert!(registry.snapshot().await.is_empty());
}

#[tokio::test]
#[serial]
async fn notifications_do_not_overlap_one_slow_join() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    Mock::given(method("POST"))
        .and(PathRegex::new(r"/join/?$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"room_id": ROOM}))
                .set_delay(Duration::from_secs(2)),
        )
        .with_priority(1)
        .mount(&homeserver)
        .await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let notify = Arc::new(tokio::sync::Notify::new());
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::new(InviteRegistry::new()),
        Arc::clone(&notify),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 1).await;
    for _ in 0..20 {
        notify.notify_one();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    handle.abort();
}

#[tokio::test]
#[serial]
async fn a_room_that_leaves_the_invite_list_is_pruned() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(OTHER)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = InviteRegistry::new();
    reconcile_pass(&client, &runtime, &registry, &RetryPolicy::default()).await;
    assert_eq!(registry.snapshot().await.len(), 1);
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(left_sync()))
        .with_priority(1)
        .mount(&homeserver)
        .await;
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("sync");
    reconcile_pass(&client, &runtime, &registry, &RetryPolicy::default()).await;
    assert!(registry.snapshot().await.is_empty());
}

#[tokio::test]
#[serial]
async fn an_untrusted_invite_stays_unjoined_when_the_bot_is_on() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(OTHER)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = Arc::new(InviteRegistry::new());
    let server = MatrixMcpServer::new(client.clone(), test_config(&homeserver.uri()))
        .with_invites(Arc::clone(&registry));
    let handle = spawn_reconciler(
        client,
        runtime,
        Arc::clone(&registry),
        Arc::new(tokio::sync::Notify::new()),
        fast_policy(),
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let body = tool_json(&server).await;
    assert_eq!(body["invitations"][0]["trusted"], "false");
    assert_eq!(body["invitations"][0]["attempts"], 0);
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 0);
    handle.abort();
}

#[tokio::test]
#[serial]
async fn an_already_joined_room_is_not_joined_again() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(TRUSTED)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let notify = Arc::new(tokio::sync::Notify::new());
    let handle = spawn_reconciler(
        client.clone(),
        runtime,
        Arc::new(InviteRegistry::new()),
        Arc::clone(&notify),
        fast_policy(),
    );
    wait_for_joins(&homeserver, 1).await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(joined_sync()))
        .with_priority(1)
        .mount(&homeserver)
        .await;
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("sync");
    notify.notify_one();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(count_path(&homeserver, "POST", "/join").await, 1);
    assert_eq!(
        count_path(&homeserver, "PUT", "/send/m.room.message/").await,
        1
    );
    handle.abort();
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn decision_logs_omit_secrets_and_repeat_lines() {
    let homeserver = MockServer::start().await;
    mount_homeserver(&homeserver, invite_sync(OTHER)).await;
    let amctl = trusted_server(json!([{ "matrix_user_id": TRUSTED }]), 200).await;
    let client = synced_client(&homeserver).await;
    let runtime = runtime_for(&session(amctl.uri(), Vec::new())).expect("runtime");
    let registry = InviteRegistry::new();
    let logs = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(LogWriter(Arc::clone(&logs)))
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    let dispatch = tracing::Dispatch::new(subscriber);
    let _guard = tracing::dispatcher::set_default(&dispatch);
    tracing::callsite::rebuild_interest_cache();
    reconcile_pass(&client, &runtime, &registry, &RetryPolicy::default()).await;
    reconcile_pass(&client, &runtime, &registry, &RetryPolicy::default()).await;
    let text = String::from_utf8(logs.lock().expect("logs").clone()).expect("utf8");
    assert!(text.contains("room_id"), "{text}");
    assert!(text.contains(ROOM));
    assert!(text.contains(OTHER));
    assert!(text.contains("skip_untrusted"));
    assert_eq!(text.matches("invite decision").count(), 1);
    assert!(!text.contains("test_access_token"));
    assert!(!text.contains("lab-test-key"));
}

struct LogWriter(Arc<std::sync::Mutex<Vec<u8>>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogWriter {
    type Writer = LogGuard;

    fn make_writer(&'a self) -> Self::Writer {
        LogGuard(Arc::clone(&self.0))
    }
}

struct LogGuard(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("logs").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
