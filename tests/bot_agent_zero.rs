//! Agent Zero turns against a local wiremock server. No Matrix room send.

use std::sync::{Arc, Mutex};

use matrix_mcp_server_r2::bot::agent_zero::{complete_turn, AgentZeroError, AgentZeroTurn};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const USER: &str = "@agent0-5:example.com";

fn turn(api_url: String, state_dir: std::path::PathBuf, prompt: &str) -> AgentZeroTurn {
    AgentZeroTurn {
        api_url,
        state_dir,
        room_id: "!room:example.com".to_string(),
        sender: "@other:example.com".to_string(),
        prompt: prompt.to_string(),
        identity: "Agent Zero".to_string(),
        user_id: USER.to_string(),
    }
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("am-a0-{label}-{nanos}"));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

fn write_key(dir: &std::path::Path, key: &str) {
    std::fs::write(dir.join(".a0-api-key"), key).expect("key file");
}

#[tokio::test]
async fn first_turn_sends_an_empty_context_and_stores_the_returned_id() {
    let server = MockServer::start().await;
    let dir = temp_dir("first");
    write_key(&dir, "key-first");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_handler = Arc::clone(&seen);
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(move |request: &Request| {
            seen_handler
                .lock()
                .expect("seen")
                .push(request_view(request));
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "the sky is blue",
                "context_id": "ctx-9"
            }))
        })
        .mount(&server)
        .await;

    let reply = complete_turn(&turn(
        format!("{}/api/api_message", server.uri()),
        dir.clone(),
        "what is the sky",
    ))
    .await
    .expect("reply");
    assert_eq!(reply, "the sky is blue");

    let seen = seen.lock().expect("seen");
    assert_eq!(
        seen.len(),
        1,
        "one POST, and this test sends no room message"
    );
    assert_eq!(seen[0].context_id, "");
    assert_eq!(seen[0].api_key, "key-first");
    assert!(seen[0].message.contains("do not send messages via MCP"));
    assert!(seen[0].message.contains("From: @other:example.com"));
    assert!(seen[0].message.contains("Room: !room:example.com"));
    assert!(seen[0].message.contains("what is the sky"));
    assert!(!seen[0].raw_body.contains("key-first"));

    let stored = std::fs::read_to_string(dir.join("room_contexts.json")).expect("contexts");
    let map: serde_json::Value = serde_json::from_str(&stored).expect("json");
    assert_eq!(map["!room:example.com"], "ctx-9");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn missing_context_retries_once_with_a_fresh_context_and_stores_it() {
    let server = MockServer::start().await;
    let dir = temp_dir("missing-context");
    write_key(&dir, "key-context");
    std::fs::write(
        dir.join("room_contexts.json"),
        serde_json::to_vec(&serde_json::json!({"!room:example.com": "ctx-expired"}))
            .expect("contexts"),
    )
    .expect("write contexts");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_handler = Arc::clone(&seen);
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(move |request: &Request| {
            let attempt = {
                let mut seen = seen_handler.lock().expect("seen");
                seen.push(request_view(request));
                seen.len()
            };
            if attempt == 1 {
                ResponseTemplate::new(404)
                    .set_body_json(serde_json::json!({"error": "Context not found"}))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "response": "fresh context reply",
                    "context_id": "ctx-fresh"
                }))
            }
        })
        .mount(&server)
        .await;

    let reply = complete_turn(&turn(
        format!("{}/api/api_message", server.uri()),
        dir.clone(),
        "try again",
    ))
    .await
    .expect("fresh context reply");
    assert_eq!(reply, "fresh context reply");
    let seen = seen.lock().expect("seen");
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].context_id, "ctx-expired");
    assert_eq!(seen[1].context_id, "");
    assert_eq!(seen[0].message, seen[1].message);
    let stored = std::fs::read_to_string(dir.join("room_contexts.json")).expect("contexts");
    let map: serde_json::Value = serde_json::from_str(&stored).expect("json");
    assert_eq!(map["!room:example.com"], "ctx-fresh");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn unrelated_404_does_not_retry_or_clear_the_saved_context() {
    let server = MockServer::start().await;
    let dir = temp_dir("unrelated-404");
    write_key(&dir, "key-404");
    std::fs::write(
        dir.join("room_contexts.json"),
        serde_json::to_vec(&serde_json::json!({"!room:example.com": "ctx-existing"}))
            .expect("contexts"),
    )
    .expect("write contexts");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_handler = Arc::clone(&seen);
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(move |request: &Request| {
            seen_handler
                .lock()
                .expect("seen")
                .push(request_view(request));
            ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not Found"}))
        })
        .mount(&server)
        .await;

    let error = complete_turn(&turn(
        format!("{}/api/api_message", server.uri()),
        dir.clone(),
        "what failed",
    ))
    .await
    .expect_err("unrelated 404");
    assert_eq!(error, AgentZeroError::AgentZeroUnavailable);
    assert_eq!(seen.lock().expect("seen").len(), 1);
    let stored = std::fs::read_to_string(dir.join("room_contexts.json")).expect("contexts");
    let map: serde_json::Value = serde_json::from_str(&stored).expect("json");
    assert_eq!(map["!room:example.com"], "ctx-existing");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn http_401_rereads_the_key_once_and_retries_once() {
    let server = MockServer::start().await;
    let dir = temp_dir("retry");
    write_key(&dir, "key-old");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_handler = Arc::clone(&seen);
    let key_dir = dir.clone();
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(move |request: &Request| {
            let view = request_view(request);
            let attempt = {
                let mut seen = seen_handler.lock().expect("seen");
                seen.push(view.clone());
                seen.len()
            };
            if attempt == 1 {
                std::fs::write(key_dir.join(".a0-api-key"), "key-new").expect("rotate key");
                ResponseTemplate::new(401)
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "response": "after retry",
                    "context_id": "ctx-retry"
                }))
            }
        })
        .mount(&server)
        .await;

    let reply = complete_turn(&turn(
        format!("{}/api/api_message", server.uri()),
        dir.clone(),
        "try again",
    ))
    .await
    .expect("retry reply");
    assert_eq!(reply, "after retry");
    let seen = seen.lock().expect("seen");
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].api_key, "key-old");
    assert_eq!(seen[1].api_key, "key-new");
    let stored = std::fs::read_to_string(dir.join("room_contexts.json")).expect("contexts");
    assert!(stored.contains("ctx-retry"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_failed_post_has_no_reply_body() {
    let server = MockServer::start().await;
    let dir = temp_dir("fail");
    write_key(&dir, "key-fail");
    Mock::given(method("POST"))
        .and(path("/api/api_message"))
        .respond_with(ResponseTemplate::new(500).set_body_string("do-not-repeat-this-body"))
        .mount(&server)
        .await;

    let error = complete_turn(&turn(
        format!("{}/api/api_message", server.uri()),
        dir.clone(),
        "what failed",
    ))
    .await
    .expect_err("failed post");
    assert_eq!(error, AgentZeroError::AgentZeroUnavailable);
    let rendered = error.to_string();
    assert!(!rendered.contains("do-not-repeat-this-body"));
    assert!(!dir.join("room_contexts.json").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[derive(Clone)]
struct SeenRequest {
    context_id: String,
    message: String,
    api_key: String,
    raw_body: String,
}

fn request_view(request: &Request) -> SeenRequest {
    let raw_body = String::from_utf8_lossy(&request.body).to_string();
    let value: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
    SeenRequest {
        context_id: value
            .get("context_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        message: value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        api_key: request
            .headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string(),
        raw_body,
    }
}
