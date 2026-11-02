//! Wiremock coverage for the `e2ee` feature.
//!
//! Device-id persistence, two store paths, and the store lock live in
//! `src/matrix/client.rs` tests. Recovery-secret precedence lives in
//! `src/matrix/secrets.rs` tests. This file covers the tool paths those
//! units do not reach: keys-query preflight, bootstrap errors, backup
//! progress, and unable-to-decrypt reads.

#![cfg(feature = "e2ee")]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use matrix_mcp_server_r2::config::Config;
use matrix_mcp_server_r2::matrix::client::create_matrix_client;
use matrix_mcp_server_r2::matrix::e2ee::bootstrap_progress;
use matrix_mcp_server_r2::mcp::server::*;
use matrix_sdk::{
    authentication::matrix::MatrixSession,
    config::{RequestConfig, SyncSettings},
    store::RoomLoadSettings,
    Client, SessionMeta, SessionTokens,
};
use rmcp::handler::server::wrapper::Parameters;
use ruma::{OwnedDeviceId, OwnedUserId};
use serde_json::{json, Value};
use serial_test::serial;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const USER: &str = "@bot:localhost";
const TOKEN: &str = "test_access_token";
const SECRET: &str = "planted-recovery-secret";
const PASSWORD: &str = "uiaa-password-sentinel";

struct Call {
    method: String,
    path: String,
    body: String,
}

struct Homeserver {
    calls: Mutex<Vec<Call>>,
    keys_query_status: AtomicU16,
    signing_upload_status: AtomicU16,
    /// After the first backup-version POST, later GETs report that a backup exists.
    backup_created: AtomicBool,
    room_encrypted: bool,
    messages: Value,
}

impl Homeserver {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            keys_query_status: AtomicU16::new(200),
            signing_upload_status: AtomicU16::new(200),
            backup_created: AtomicBool::new(false),
            room_encrypted: false,
            messages: json!({ "start": "s", "end": "e", "chunk": [] }),
        })
    }

    fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .expect("call log")
            .iter()
            .map(|call| Call {
                method: call.method.clone(),
                path: call.path.clone(),
                body: call.body.clone(),
            })
            .collect()
    }

    fn count(&self, method: &str, path_part: &str) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.method == method && call.path.contains(path_part))
            .count()
    }
}

struct Shared(Arc<Homeserver>);

impl Respond for Shared {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let method = request.method.as_str().to_string();
        let path = request.url.path().to_string();
        let body = String::from_utf8_lossy(&request.body).into_owned();
        self.0.calls.lock().expect("call log").push(Call {
            method: method.clone(),
            path: path.clone(),
            body,
        });
        route(&self.0, &method, &path)
    }
}

fn route(server: &Homeserver, method: &str, path: &str) -> ResponseTemplate {
    if method == "GET" && path.ends_with("/versions") {
        return ResponseTemplate::new(200).set_body_json(json!({
            "versions": ["v1.11"],
            "unstable_features": {}
        }));
    }
    if method == "GET" && path.contains("/sync") {
        return ResponseTemplate::new(200).set_body_json(sync_body());
    }
    if method == "POST" && path.contains("/filter") {
        return ResponseTemplate::new(200).set_body_json(json!({ "filter_id": "0" }));
    }
    if method == "GET" && path.ends_with("/capabilities") {
        return ResponseTemplate::new(200).set_body_json(json!({ "capabilities": {} }));
    }
    if method == "GET" && path.contains("/pushrules") {
        return ResponseTemplate::new(200).set_body_json(json!({ "global": {} }));
    }
    if method == "GET" && path.ends_with("/whoami") {
        return ResponseTemplate::new(200).set_body_json(json!({
            "user_id": USER,
            "device_id": "DEVICE_FROM_WHOAMI"
        }));
    }
    if method == "GET" && path.contains("/state/m.room.encryption") {
        if server.room_encrypted {
            return ResponseTemplate::new(200).set_body_json(json!({
                "algorithm": "m.megolm.v1.aes-sha2"
            }));
        }
        return ResponseTemplate::new(404).set_body_json(json!({
            "errcode": "M_NOT_FOUND",
            "error": "Event not found."
        }));
    }
    if method == "POST" && path.ends_with("/keys/query") {
        let status = server.keys_query_status.load(Ordering::SeqCst);
        if status != 200 {
            return ResponseTemplate::new(status).set_body_json(json!({
                "errcode": "M_UNKNOWN",
                "error": "keys query failed"
            }));
        }
        return ResponseTemplate::new(200).set_body_json(keys_query_body(server));
    }
    if method == "POST" && path.contains("/keys/device_signing/upload") {
        let status = server.signing_upload_status.load(Ordering::SeqCst);
        if status == 401 {
            return ResponseTemplate::new(401).set_body_json(json!({
                "session": "uiaa-session",
                "flows": [{ "stages": ["m.login.password"] }],
                "params": {},
                "completed": []
            }));
        }
        return ResponseTemplate::new(200).set_body_json(json!({}));
    }
    if method == "POST" && path.contains("/keys/signatures/upload") {
        return ResponseTemplate::new(200).set_body_json(json!({ "failures": {} }));
    }
    if method == "POST" && path.contains("/keys/upload") {
        return ResponseTemplate::new(200).set_body_json(json!({ "one_time_key_counts": {} }));
    }
    if path.contains("/room_keys/version") && method == "GET" {
        let created = server.backup_created.load(Ordering::SeqCst);
        let template = if created {
            ResponseTemplate::new(200).set_body_json(json!({
                "algorithm": "m.megolm_backup.v1.curve25519-aes-sha2",
                "auth_data": {},
                "version": "1"
            }))
        } else {
            ResponseTemplate::new(404).set_body_json(json!({
                "errcode": "M_NOT_FOUND",
                "error": "No backup"
            }))
        };
        return template.set_delay(Duration::from_millis(300));
    }
    if path.contains("/room_keys/version") && method == "POST" {
        server.backup_created.store(true, Ordering::SeqCst);
        return ResponseTemplate::new(200).set_body_json(json!({ "version": "1" }));
    }
    if method == "POST" && path.ends_with("/createRoom") {
        return ResponseTemplate::new(200)
            .set_body_json(json!({ "room_id": "!created:localhost" }));
    }
    if method == "GET" && path.contains("/messages") {
        return ResponseTemplate::new(200).set_body_json(server.messages.clone());
    }
    if method == "PUT" && path.contains("/send/") {
        return ResponseTemplate::new(200).set_body_json(json!({ "event_id": "$sent-e2ee" }));
    }
    ResponseTemplate::new(200).set_body_json(json!({}))
}

fn keys_query_body(server: &Homeserver) -> Value {
    let mut body = json!({
        "device_keys": {},
        "failures": {},
        "master_keys": {},
        "self_signing_keys": {},
        "user_signing_keys": {}
    });
    if let Some(upload) = server
        .calls()
        .into_iter()
        .rev()
        .find(|call| call.path.contains("/keys/device_signing/upload") && call.method == "POST")
    {
        if let Ok(uploaded) = serde_json::from_str::<Value>(&upload.body) {
            for (field, target) in [
                ("master_key", "master_keys"),
                ("self_signing_key", "self_signing_keys"),
                ("user_signing_key", "user_signing_keys"),
            ] {
                if let Some(key) = uploaded.get(field) {
                    if let Some(user) = key.get("user_id").and_then(Value::as_str) {
                        body[target][user] = key.clone();
                    }
                }
            }
        }
    }
    if let Some(device) = signed_device(server) {
        let user = device
            .get("user_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let device_id = device
            .get("device_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let (Some(user), Some(device_id)) = (user, device_id) {
            body["device_keys"][user][device_id] = device;
        }
    }
    body
}

/// Device keys from the signatures upload, plus the device's own signature
/// from the matching `/keys/upload` body.
fn signed_device(server: &Homeserver) -> Option<Value> {
    let calls = server.calls();
    let mut device = None;
    for call in &calls {
        if call.method == "POST" && call.path.contains("/keys/signatures/upload") {
            let uploaded: Value = serde_json::from_str(&call.body).ok()?;
            device = uploaded.get(USER)?.get("TEST_DEVICE").cloned();
        }
    }
    let mut device = device?;
    let curve = device.pointer("/keys/curve25519:TEST_DEVICE").cloned();
    for call in calls.iter().rev() {
        if call.method == "POST"
            && call.path.contains("/keys/upload")
            && !call.path.contains("device_signing")
            && !call.path.contains("signatures")
        {
            let Ok(uploaded) = serde_json::from_str::<Value>(&call.body) else {
                continue;
            };
            let Some(uploaded_device) = uploaded.get("device_keys") else {
                continue;
            };
            if uploaded_device.pointer("/keys/curve25519:TEST_DEVICE") != curve.as_ref() {
                continue;
            }
            if let Some(signature) =
                uploaded_device.pointer(&format!("/signatures/{USER}/ed25519:TEST_DEVICE"))
            {
                device["signatures"][USER]["ed25519:TEST_DEVICE"] = signature.clone();
            }
            break;
        }
    }
    Some(device)
}

fn sync_body() -> Value {
    json!({
        "next_batch": "s1",
        "rooms": {
            "join": {
                "!testroom:localhost": {
                    "timeline": { "events": [], "prev_batch": "p", "limited": false },
                    "state": { "events": [
                        {
                            "type": "m.room.create",
                            "state_key": "",
                            "event_id": "$create",
                            "sender": USER,
                            "origin_server_ts": 1,
                            "content": { "creator": USER, "room_version": "10" }
                        },
                        {
                            "type": "m.room.member",
                            "state_key": USER,
                            "event_id": "$join",
                            "sender": USER,
                            "origin_server_ts": 1,
                            "content": { "membership": "join" }
                        }
                    ] },
                    "account_data": { "events": [] },
                    "ephemeral": { "events": [] },
                    "unread_notifications": { "notification_count": 0, "highlight_count": 0 }
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

async fn mount(server: Arc<Homeserver>) -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(Shared(server))
        .mount(&mock)
        .await;
    mock
}

fn config(
    url: &str,
    e2ee: bool,
    skip: bool,
    recovery: Option<String>,
    uiaa: Option<String>,
) -> Arc<Config> {
    Arc::new(Config {
        matrix_homeserver_url: url.to_string(),
        matrix_access_token: TOKEN.to_string(),
        matrix_user_id: USER.to_string(),
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
        e2ee_enabled: e2ee,
        crypto_store_path: None,
        recovery_key_file: recovery,
        uiaa_password_file: uiaa,
        debug: false,
        skip_matrix_init: skip,
    })
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "matrix-mcp-e2ee-{label}-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

async fn sqlite_client(mock: &MockServer, store: &std::path::Path) -> Client {
    let client = Client::builder()
        .homeserver_url(mock.uri())
        .request_config(RequestConfig::new().disable_retry())
        .sqlite_store(store, None)
        .build()
        .await
        .expect("sqlite client");
    client
        .matrix_auth()
        .restore_session(
            MatrixSession {
                meta: SessionMeta {
                    user_id: OwnedUserId::try_from(USER).unwrap(),
                    device_id: OwnedDeviceId::from("TEST_DEVICE"),
                },
                tokens: SessionTokens {
                    access_token: TOKEN.to_string(),
                    refresh_token: None,
                },
            },
            RoomLoadSettings::default(),
        )
        .await
        .expect("restore");
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(3)))
        .await
        .expect("sync");
    client
}

fn tool_text(result: &rmcp::model::CallToolResult) -> String {
    format!("{result:?}")
}

fn create_input(mode: &str, invite: Option<&str>) -> Parameters<CreateRoomInput> {
    Parameters(CreateRoomInput {
        room_name: "Encrypted or not".into(),
        is_private: Some(true),
        topic: None,
        invite_users: invite.map(|user| vec![user.to_string()]),
        room_alias: None,
        encryption_mode: Some(mode.into()),
    })
}

#[tokio::test]
async fn keys_query_failure_does_not_create_a_room() {
    let homeserver = Homeserver::new();
    homeserver.keys_query_status.store(500, Ordering::SeqCst);
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("query-fail");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .create_room(create_input("auto", Some("@alice:localhost")))
        .await
        .expect("tool");
    let text = tool_text(&result);
    assert!(
        text.contains("keys query failed"),
        "expected fail closed: {text}"
    );
    assert_eq!(
        homeserver.count("POST", "/createRoom"),
        0,
        "room was created: {text}"
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
async fn auto_mode_falls_back_when_the_invitee_has_no_keys() {
    let homeserver = Homeserver::new();
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("fallback");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .create_room(create_input("auto", Some("@alice:localhost")))
        .await
        .expect("tool");
    let text = tool_text(&result);
    assert!(
        text.contains("Encrypted: No (fallback: @alice:localhost has no cross-signed E2EE keys)"),
        "fallback notice missing: {text}"
    );
    let created = homeserver
        .calls()
        .into_iter()
        .find(|call| call.path.ends_with("/createRoom"));
    let created = created.expect("createRoom was not called");
    assert!(
        !created.body.contains("m.room.encryption"),
        "fallback room was encrypted: {}",
        created.body
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
async fn e2ee_mode_rejects_an_invitee_without_keys() {
    let homeserver = Homeserver::new();
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("strict");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .create_room(create_input("e2ee", Some("@alice:localhost")))
        .await
        .expect("tool");
    let text = tool_text(&result);
    assert!(
        text.contains("TrustPolicyNotSatisfiedError"),
        "expected rejection: {text}"
    );
    assert!(text.contains("@alice:localhost"), "user id missing: {text}");
    assert_eq!(homeserver.count("POST", "/createRoom"), 0);
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
async fn unable_to_decrypt_events_stay_in_the_reader() {
    let homeserver = Homeserver::new();
    let event = json!({
        "type": "m.room.encrypted",
        "event_id": "$enc1",
        "sender": "@alice:localhost",
        "origin_server_ts": 1700000000000u64,
        "content": {
            "algorithm": "m.megolm.v1.aes-sha2",
            "ciphertext": "AwgAEiBQs2LgBD2CcB+RLH2bsgp9VadFUJhBXOtCmcJuttBDOeDNjL21d9z0AcVSfQFAh9huh4or7sWuNrHcvu9/sMbweTgc0UtdA5xFLheubHouXy4aewze+ShndWAaTbjWJMLsPSQDUMQHBA",
            "sender_key": "Nn0L2hkcCMFKqynTjyGsJbth7QrVmX3lbrksMkrGOAw",
            "session_id": "ZFD6+OmV7fVCsJ7Gap8UnORH8EnmiAkes8FAvQuCw/I",
            "device_id": "ALICEDEVICE"
        }
    });
    // The messages body is fixed at construction. Rebuild with the event.
    let homeserver = Arc::new(Homeserver {
        messages: json!({ "start": "s", "end": "e", "chunk": [event] }),
        ..wait_rebuild(homeserver)
    });
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("utd");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .get_room_messages(Parameters(GetRoomMessagesInput {
            room_id: "!testroom:localhost".into(),
            limit: Some(10),
        }))
        .await
        .expect("messages");
    let text = tool_text(&result);
    assert!(
        text.contains("[Encrypted message - unable to decrypt (missing_key)]"),
        "UTD marker missing: {text}"
    );
    assert!(text.contains("@alice:localhost"), "sender missing: {text}");
    let _ = std::fs::remove_dir_all(&store);
}

fn wait_rebuild(server: Arc<Homeserver>) -> Homeserver {
    Homeserver {
        calls: Mutex::new(Vec::new()),
        keys_query_status: AtomicU16::new(server.keys_query_status.load(Ordering::SeqCst)),
        signing_upload_status: AtomicU16::new(server.signing_upload_status.load(Ordering::SeqCst)),
        backup_created: AtomicBool::new(false),
        room_encrypted: server.room_encrypted,
        messages: json!({ "start": "s", "end": "e", "chunk": [] }),
    }
}

#[tokio::test]
async fn encrypted_send_without_a_cross_signed_device_does_not_send() {
    let homeserver = Arc::new(Homeserver {
        room_encrypted: true,
        ..wait_rebuild(Homeserver::new())
    });
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("send-guard");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .send_message(Parameters(SendMessageInput {
            room_id: "!testroom:localhost".into(),
            message: "should not leave".into(),
            message_type: None,
            reply_to_event_id: None,
        }))
        .await
        .expect("send");
    let text = tool_text(&result);
    assert!(
        text.contains("CryptoNotBootstrappedError"),
        "expected bootstrap error: {text}"
    );
    assert_eq!(
        homeserver.count("PUT", "/send/"),
        0,
        "plaintext send left the process: {text}"
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
#[serial]
async fn tofu_is_rejected_and_the_policy_value_is_not_echoed() {
    let homeserver = Homeserver::new();
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("tofu");
    let client = sqlite_client(&mock, &store).await;
    let before = homeserver.calls().len();
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
            force_recreate: false,
            trust_policy: Some("tofu".into()),
            backup_passphrase_env_var: None,
            backup_passphrase_ref: None,
        }))
        .await
        .expect("bootstrap");
    let text = tool_text(&result);
    assert!(
        text.contains("TrustPolicyNotSatisfiedError"),
        "expected rejection: {text}"
    );
    assert!(!text.contains("tofu"), "rejected policy was echoed: {text}");
    assert_eq!(
        homeserver.calls().len(),
        before,
        "tofu reached the homeserver"
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
#[serial]
async fn passphrase_ref_error_omits_the_secret_from_the_tool_result() {
    let homeserver = Homeserver::new();
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("secret-ref");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, false, None, None));
    let result = server
        .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
            force_recreate: false,
            trust_policy: Some("cross-signed".into()),
            backup_passphrase_env_var: None,
            backup_passphrase_ref: Some(SECRET.into()),
        }))
        .await
        .expect("bootstrap");
    let text = tool_text(&result);
    assert!(
        text.contains("SecretResolutionError"),
        "expected resolution error: {text}"
    );
    assert!(
        !text.contains(SECRET),
        "secret leaked into the tool result: {text}"
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
#[serial]
async fn uiaa_without_a_readable_password_does_not_upload_an_identity() {
    let homeserver = Homeserver::new();
    homeserver
        .signing_upload_status
        .store(401, Ordering::SeqCst);
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("uiaa");
    let password_path = store.join("password");
    std::fs::write(&password_path, PASSWORD).expect("password file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&password_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(
        client,
        config(
            &mock.uri(),
            true,
            false,
            Some(store.join("recovery-key").display().to_string()),
            Some(password_path.display().to_string()),
        ),
    );
    let result = server
        .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
            force_recreate: false,
            trust_policy: None,
            backup_passphrase_env_var: None,
            backup_passphrase_ref: None,
        }))
        .await
        .expect("bootstrap");
    let text = tool_text(&result);
    assert!(
        text.contains("UiaaRequiredError"),
        "expected UIAA error: {text}"
    );
    assert!(!text.contains(PASSWORD), "password leaked: {text}");
    assert_eq!(
        homeserver.count("POST", "/keys/device_signing/upload"),
        1,
        "identity upload was retried"
    );
    assert!(
        !store.join("recovery-key").exists(),
        "recovery key written after UIAA failure"
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
#[serial]
async fn skip_matrix_init_matches_the_other_tools() {
    let homeserver = Homeserver::new();
    let mock = mount(homeserver).await;
    let store = temp_dir("skip");
    let client = sqlite_client(&mock, &store).await;
    let server = MatrixMcpServer::new(client, config(&mock.uri(), true, true, None, None));
    let bootstrap = tool_text(
        &server
            .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
                force_recreate: false,
                trust_policy: None,
                backup_passphrase_env_var: None,
                backup_passphrase_ref: None,
            }))
            .await
            .expect("bootstrap"),
    );
    let status = tool_text(
        &server
            .get_encryption_status(Parameters(GetEncryptionStatusInput { room_id: None }))
            .await
            .expect("status"),
    );
    assert!(bootstrap.contains("Matrix not initialized"), "{bootstrap}");
    assert!(status.contains("Matrix not initialized"), "{status}");
    let _ = std::fs::remove_dir_all(&store);
}

#[tokio::test]
#[serial]
async fn bootstrap_reports_running_then_finishes_and_a_fresh_store_can_see_the_identity() {
    let homeserver = Homeserver::new();
    let mock = mount(Arc::clone(&homeserver)).await;
    let store = temp_dir("bootstrap");
    let recovery = store.join("recovery-key");
    let client = sqlite_client(&mock, store.join("store-a").as_path()).await;
    let server = MatrixMcpServer::new(
        client,
        config(
            &mock.uri(),
            true,
            false,
            Some(recovery.display().to_string()),
            None,
        ),
    );
    let result = server
        .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
            force_recreate: false,
            trust_policy: None,
            backup_passphrase_env_var: None,
            backup_passphrase_ref: None,
        }))
        .await
        .expect("bootstrap");
    let text = tool_text(&result);
    assert!(
        text.contains("keyBackupRestore") && (text.contains("running") || text.contains("done")),
        "restore state missing: {text}\ncalls: {:?}",
        homeserver
            .calls()
            .iter()
            .map(|call| format!("{} {}", call.method, call.path))
            .collect::<Vec<_>>()
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut restore = bootstrap_progress().key_backup_restore;
    while restore == "running" && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
        restore = bootstrap_progress().key_backup_restore;
    }
    assert!(
        restore == "done" || restore == "failed",
        "restore stuck at {restore}"
    );

    let mode = std::fs::metadata(&recovery)
        .expect("recovery key")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let key_bytes = std::fs::read(&recovery).expect("recovery bytes");
    assert!(!text.contains(&String::from_utf8_lossy(&key_bytes).into_owned()));

    // A second store, no secret, against the identity this bootstrap uploaded.
    let fresh = temp_dir("fresh-identity");
    let fresh_client = sqlite_client(&mock, &fresh).await;
    let fresh_server = MatrixMcpServer::new(
        fresh_client,
        config(
            &mock.uri(),
            true,
            false,
            Some(fresh.join("missing-key").display().to_string()),
            None,
        ),
    );
    let again = tool_text(
        &fresh_server
            .bootstrap_e2ee(Parameters(BootstrapE2eeInput {
                force_recreate: false,
                trust_policy: None,
                backup_passphrase_env_var: None,
                backup_passphrase_ref: None,
            }))
            .await
            .expect("second bootstrap"),
    );
    assert!(
        again.contains("IdentityExistsError"),
        "expected existing identity: {again}\nquery bodies: {:?}",
        homeserver
            .calls()
            .iter()
            .filter(|call| call.path.ends_with("/keys/query"))
            .map(|call| call.body.clone())
            .collect::<Vec<_>>()
    );

    let created = tool_text(
        &server
            .create_room(create_input("auto", Some(USER)))
            .await
            .expect("create"),
    );
    assert!(
        created.contains("Encrypted: Yes"),
        "cross-signed invitee should produce an encrypted room: {created}"
    );
    let room = homeserver
        .calls()
        .into_iter()
        .rev()
        .find(|call| call.path.ends_with("/createRoom"))
        .expect("createRoom");
    assert!(
        room.body.contains("m.room.encryption"),
        "initial state omitted encryption: {}",
        room.body
    );
    assert!(
        room.body.contains("m.room.history_visibility"),
        "history visibility missing"
    );

    let _ = std::fs::remove_dir_all(&store);
    let _ = std::fs::remove_dir_all(&fresh);
}

#[tokio::test]
async fn two_whoami_devices_land_in_two_store_paths() {
    let first_mock_state = Homeserver::new();
    let first_mock = mount(first_mock_state).await;
    let first_store = temp_dir("whoami-a");
    let first = create_matrix_client(
        &first_mock.uri(),
        USER,
        TOKEN,
        Some(&first_store),
        Some(&first_store.join("recovery-key")),
        None,
    )
    .await;
    let first_id =
        std::fs::read_to_string(first_store.join(".device-id")).expect("first device file");
    drop(first);
    let second_store = temp_dir("whoami-b");
    // Same whoami mock returns one device id. A second path still gets its own file
    // with that id, and reopening the first path keeps the original line.
    let second_mock_state = Homeserver::new();
    let second_mock = mount(second_mock_state).await;
    let _second = create_matrix_client(
        &second_mock.uri(),
        USER,
        TOKEN,
        Some(&second_store),
        Some(&second_store.join("recovery-key")),
        None,
    )
    .await;
    let second_id =
        std::fs::read_to_string(second_store.join(".device-id")).expect("second device file");
    assert_eq!(first_id.trim(), "DEVICE_FROM_WHOAMI");
    assert_eq!(second_id.trim(), "DEVICE_FROM_WHOAMI");
    let reopened = std::fs::read_to_string(first_store.join(".device-id")).unwrap();
    assert_eq!(reopened.trim(), first_id.trim());
    let _ = std::fs::remove_dir_all(&first_store);
    let _ = std::fs::remove_dir_all(&second_store);
}
