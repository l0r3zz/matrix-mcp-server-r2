//! One image on a triggered prompt, against wiremock.
//!
//! The homeserver is local. The evaluator is a second wiremock server.
//! No encrypted-file download is required here.

use matrix_mcp_server_r2::bot::{self, Session};
use matrix_sdk::{
    authentication::matrix::MatrixSession, config::SyncSettings, store::RoomLoadSettings, Client,
    SessionMeta, SessionTokens,
};
use ruma::{OwnedDeviceId, OwnedUserId};
use serde_json::{json, Value};
use serial_test::serial;
use std::sync::Mutex;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const SESSION_USER: &str = "@agent0-5:example.com";
const OTHER: &str = "@other:example.com";
const ROOM: &str = "!ops:example.com";
const TOKEN: &str = "test_access_token";
const CAPTION: &str = "agent0-5: secret-caption";

static LOGS: Mutex<String> = Mutex::new(String::new());

struct LogSink;

impl std::io::Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        LOGS.lock()
            .expect("logs")
            .push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = LogSink;

    fn make_writer(&'a self) -> Self::Writer {
        LogSink
    }
}

fn init_logs() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_writer(LogSink)
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
    LOGS.lock().expect("logs").clear();
}

fn logs() -> String {
    LOGS.lock().expect("logs").clone()
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

fn now_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    )
    .unwrap_or(0)
}

fn state_dir() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "am-image-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("state dir");
    std::fs::write(path.join(".a0-api-key"), "lab-test-key").expect("key");
    path
}

fn session(api_url: String, accepts_images: bool) -> Session {
    Session {
        user_id: SESSION_USER.to_string(),
        display_name: String::new(),
        identity: "Agent Zero".to_string(),
        state_dir: state_dir(),
        a0_api_url: api_url,
        amctl_url: "http://127.0.0.1:9".to_string(),
        amctl_api_key: None,
        trusted_senders: Vec::new(),
        accepts_images,
    }
}

fn image_event(content: Value) -> Value {
    json!({
        "type": "m.room.message",
        "event_id": "$img1",
        "sender": OTHER,
        "origin_server_ts": now_ms(),
        "content": content
    })
}

fn text_event(body: &str) -> Value {
    json!({
        "type": "m.room.message",
        "event_id": "$text1",
        "sender": OTHER,
        "origin_server_ts": now_ms(),
        "content": { "msgtype": "m.text", "body": body }
    })
}

fn joined_sync(event: Value) -> Value {
    json!({
        "next_batch": "s1",
        "rooms": {
            "join": {
                ROOM: {
                    "state": {
                        "events": [
                            {
                                "type": "m.room.create",
                                "state_key": "",
                                "sender": OTHER,
                                "content": { "room_version": "10" }
                            },
                            {
                                "type": "m.room.member",
                                "state_key": SESSION_USER,
                                "sender": SESSION_USER,
                                "content": { "membership": "join" }
                            },
                            {
                                "type": "m.room.member",
                                "state_key": OTHER,
                                "sender": OTHER,
                                "content": { "membership": "join" }
                            }
                        ]
                    },
                    "timeline": {
                        "events": [event],
                        "limited": false,
                        "prev_batch": "p"
                    }
                }
            }
        }
    })
}

async fn mount_homeserver(mock: &MockServer, sync_body: Value) {
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
    Mock::given(method("PUT"))
        .and(PathRegex::new(r"/send/m\.room\.message/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id": "$sent"})))
        .mount(mock)
        .await;
    Mock::given(method("PUT"))
        .and(PathRegex::new(r"/typing/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(mock)
        .await;
}

async fn mount_media(mock: &MockServer, status: u16, bytes: &'static [u8]) {
    Mock::given(method("GET"))
        .and(PathRegex::new(r"/download/example\.com/pic"))
        .respond_with(ResponseTemplate::new(status).set_body_bytes(bytes))
        .mount(mock)
        .await;
}

fn one_image(size: u64) -> Value {
    json!({
        "msgtype": "m.image",
        "body": CAPTION,
        "url": "mxc://example.com/pic",
        "info": { "mimetype": "image/png", "size": size }
    })
}

async fn client_for(mock: &MockServer) -> Client {
    let client = Client::builder()
        .homeserver_url(mock.uri())
        .request_config(matrix_sdk::config::RequestConfig::new().disable_retry())
        .build()
        .await
        .expect("client");
    client
        .matrix_auth()
        .restore_session(
            MatrixSession {
                meta: SessionMeta {
                    user_id: OwnedUserId::try_from(SESSION_USER).expect("user"),
                    device_id: OwnedDeviceId::from("DEVICE"),
                },
                tokens: SessionTokens {
                    access_token: TOKEN.to_string(),
                    refresh_token: None,
                },
            },
            RoomLoadSettings::default(),
        )
        .await
        .expect("session");
    client
}

async fn sync_attached(client: &Client, session: &Session) {
    bot::attach(client, session).await.expect("handlers");
    client
        .sync_once(SyncSettings::default().timeout(Duration::from_secs(2)))
        .await
        .expect("sync");
}

async fn requests(mock: &MockServer) -> Vec<Request> {
    mock.received_requests().await.expect("requests")
}

fn is_send(request: &Request) -> bool {
    request.method.as_str() == "PUT" && request.url.path().contains("/send/m.room.message/")
}

fn is_download(request: &Request) -> bool {
    request.method.as_str() == "GET" && request.url.path().contains("/download/")
}

fn is_evaluator(request: &Request) -> bool {
    request.method.as_str() == "POST" && request.url.path().contains("/api/api_message")
}

fn send_bodies(found: &[Request]) -> Vec<Value> {
    found
        .iter()
        .filter(|request| is_send(request))
        .filter_map(|request| serde_json::from_slice(&request.body).ok())
        .collect()
}

async fn wait_until(mock: &MockServer, ready: impl Fn(&[Request]) -> bool) {
    for _ in 0..40 {
        if ready(&requests(mock).await) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[serial]
async fn an_unset_flag_posts_the_capability_error_and_does_not_call_the_evaluator() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(&homeserver, joined_sync(image_event(one_image(4)))).await;
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "should not be called",
            "context_id": "c1"
        })))
        .mount(&evaluator)
        .await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), false),
    )
    .await;
    wait_until(&homeserver, |found| found.iter().any(is_send)).await;

    let bodies = send_bodies(&requests(&homeserver).await);
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["msgtype"], "m.text");
    assert_eq!(
        bodies[0]["body"],
        "This agent cannot evaluate an image. The prompt was not evaluated."
    );
    assert_eq!(
        requests(&evaluator)
            .await
            .iter()
            .filter(|request| is_evaluator(request))
            .count(),
        0
    );
    assert_eq!(
        requests(&homeserver)
            .await
            .iter()
            .filter(|request| is_download(request))
            .count(),
        0
    );
    let logged = logs();
    assert!(!logged.contains(TOKEN));
    assert!(!logged.contains("secret-caption"));
}

#[tokio::test]
#[serial]
async fn a_text_prompt_omits_attachments() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(
        &homeserver,
        joined_sync(text_event("agent0-5: hello there")),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "hello back",
            "context_id": "c1"
        })))
        .mount(&evaluator)
        .await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), false),
    )
    .await;
    wait_until(&evaluator, |found| found.iter().any(is_evaluator)).await;

    let posts: Vec<Value> = requests(&evaluator)
        .await
        .iter()
        .filter(|request| is_evaluator(request))
        .filter_map(|request| serde_json::from_slice(&request.body).ok())
        .collect();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].get("message").is_some());
    assert!(posts[0].get("context_id").is_some());
    assert!(posts[0].get("attachments").is_none());
}

#[tokio::test]
#[serial]
async fn one_ready_image_is_posted_once_and_a_failed_call_does_not_retry() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(&homeserver, joined_sync(image_event(one_image(9)))).await;
    mount_media(&homeserver, 200, b"png-bytes").await;
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "a red door",
            "context_id": "c1"
        })))
        .mount(&evaluator)
        .await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&homeserver, |found| found.iter().any(is_send)).await;

    let posts: Vec<Value> = requests(&evaluator)
        .await
        .iter()
        .filter(|request| is_evaluator(request))
        .filter_map(|request| serde_json::from_slice(&request.body).ok())
        .collect();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].get("message").is_some());
    assert!(posts[0].get("context_id").is_some());
    assert_eq!(
        posts[0]["attachments"]
            .as_array()
            .expect("attachments")
            .len(),
        1
    );
    let bodies = send_bodies(&requests(&homeserver).await);
    assert!(bodies.iter().any(|body| body["body"] == "a red door"));
    let logged = logs();
    assert!(!logged.contains(TOKEN));
    assert!(!logged.contains("cG5nLWJ5dGVz"));

    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(&homeserver, joined_sync(image_event(one_image(9)))).await;
    mount_media(&homeserver, 200, b"png-bytes").await;
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(500).set_body_string("no"))
        .mount(&evaluator)
        .await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&homeserver, |found| {
        send_bodies(found)
            .iter()
            .any(|body| body["body"] == "The model did not answer.")
    })
    .await;
    assert_eq!(
        requests(&evaluator)
            .await
            .iter()
            .filter(|request| is_evaluator(request))
            .count(),
        1
    );
    assert!(send_bodies(&requests(&homeserver).await)
        .iter()
        .any(|body| body["body"] == "The model did not answer."));
}

#[tokio::test]
#[serial]
async fn a_failed_text_prompt_posts_no_new_room_error() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(
        &homeserver,
        joined_sync(text_event("agent0-5: hello there")),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(500).set_body_string("no"))
        .mount(&evaluator)
        .await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&evaluator, |found| found.iter().any(is_evaluator)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(send_bodies(&requests(&homeserver).await)
        .iter()
        .all(|body| body["body"] != "The model did not answer."));
}

#[tokio::test]
#[serial]
async fn a_declared_oversize_image_is_not_downloaded() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(&homeserver, joined_sync(image_event(one_image(10_000_001)))).await;
    mount_media(&homeserver, 200, b"png-bytes").await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&homeserver, |found| found.iter().any(is_send)).await;
    assert_eq!(
        requests(&homeserver)
            .await
            .iter()
            .filter(|request| is_download(request))
            .count(),
        0
    );
    assert_eq!(
        requests(&evaluator)
            .await
            .iter()
            .filter(|request| is_evaluator(request))
            .count(),
        0
    );
    assert!(send_bodies(&requests(&homeserver).await)
        .iter()
        .any(|body| { body["body"] == "The image is too large. The prompt was not evaluated." }));
}

#[tokio::test]
#[serial]
async fn two_media_urls_and_a_failed_download_do_not_call_the_evaluator() {
    init_logs();
    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    let two = json!({
        "msgtype": "m.image",
        "body": CAPTION,
        "url": "mxc://example.com/pic",
        "extra": "mxc://example.com/other",
        "info": { "mimetype": "image/png", "size": 4 }
    });
    mount_homeserver(&homeserver, joined_sync(image_event(two))).await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&homeserver, |found| found.iter().any(is_send)).await;
    assert_eq!(
        requests(&evaluator)
            .await
            .iter()
            .filter(|request| is_evaluator(request))
            .count(),
        0
    );
    assert!(send_bodies(&requests(&homeserver).await)
        .iter()
        .any(|body| {
            body["body"] == "Only one image is supported. The prompt was not evaluated."
        }));

    let homeserver = MockServer::start().await;
    let evaluator = MockServer::start().await;
    mount_homeserver(&homeserver, joined_sync(image_event(one_image(4)))).await;
    mount_media(&homeserver, 404, b"").await;
    let client = client_for(&homeserver).await;
    sync_attached(
        &client,
        &session(format!("{}/api/api_message", evaluator.uri()), true),
    )
    .await;
    wait_until(&homeserver, |found| found.iter().any(is_send)).await;
    assert_eq!(
        requests(&evaluator)
            .await
            .iter()
            .filter(|request| is_evaluator(request))
            .count(),
        0
    );
    assert!(send_bodies(&requests(&homeserver).await)
        .iter()
        .any(|body| {
            body["body"] == "The image could not be read. The prompt was not evaluated."
        }));
}
