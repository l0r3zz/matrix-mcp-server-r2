//! Trusted-sender lookups against a local wiremock server. No homeserver join.

use std::sync::{Arc, Mutex};

use matrix_mcp_server_r2::bot::trusted::{
    greeting, invite_action, refresh, InviteAction, TrustMemory, TrustQuery,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const INVITEE: &str = "@agent0-5:example.com";

fn query(base_url: String, fallback: Vec<String>) -> TrustQuery {
    TrustQuery {
        amctl_url: base_url,
        api_key: Some("lab-test-key".to_string()),
        fallback,
    }
}

#[tokio::test]
async fn a_non_empty_amctl_array_replaces_the_set() {
    let server = MockServer::start().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_handler = Arc::clone(&seen);
    Mock::given(method("GET"))
        .and(path("/api/v1/trusted-senders"))
        .respond_with(move |request: &Request| {
            seen_handler.lock().expect("seen").push(
                request
                    .headers
                    .get("x-api-key")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string(),
            );
            ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "matrix_user_id": INVITEE }
            ]))
        })
        .mount(&server)
        .await;

    let mut memory = TrustMemory::from_ids(["stale-entry"]);
    refresh(&mut memory, &query(server.uri(), Vec::new())).await;
    assert_eq!(seen.lock().expect("seen").as_slice(), ["lab-test-key"]);
    assert!(memory.admits(INVITEE));
    assert!(!memory.ids().contains("stale-entry"));
    assert_eq!(invite_action(&memory, INVITEE), InviteAction::Join);
    assert_eq!(
        greeting("Galadriel"),
        "Hello! I'm Galadriel, an AI agent powered by Agent Zero."
    );
}

#[tokio::test]
async fn a_failed_get_with_an_empty_fallback_does_not_join() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/trusted-senders"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let mut memory = TrustMemory::default();
    refresh(&mut memory, &query(server.uri(), Vec::new())).await;
    assert!(memory.ids().is_empty());
    assert_eq!(invite_action(&memory, INVITEE), InviteAction::Reject);
}

#[tokio::test]
async fn a_failed_get_uses_the_fallback_only_when_the_set_is_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/trusted-senders"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let mut empty = TrustMemory::default();
    refresh(&mut empty, &query(server.uri(), vec![INVITEE.to_string()])).await;
    assert_eq!(invite_action(&empty, INVITEE), InviteAction::Join);

    let mut kept = TrustMemory::from_ids(["already-loaded"]);
    refresh(&mut kept, &query(server.uri(), vec![INVITEE.to_string()])).await;
    assert!(kept.ids().contains("already-loaded"));
    assert!(!kept.admits(INVITEE));
    assert_eq!(invite_action(&kept, INVITEE), InviteAction::Reject);
}

#[test]
fn the_server_binary_has_no_compiled_user_id_or_api_key() {
    let path = option_env!("CARGO_BIN_EXE_matrix_mcp_server_r2")
        .or(option_env!("CARGO_BIN_EXE_matrix-mcp-server-r2"))
        .unwrap_or_else(|| {
            panic!(
                "Cargo did not pass the server binary path. underscore={:?} hyphen={:?}",
                option_env!("CARGO_BIN_EXE_matrix_mcp_server_r2"),
                option_env!("CARGO_BIN_EXE_matrix-mcp-server-r2")
            )
        });
    let bytes = std::fs::read(path).expect("server binary");
    let text = String::from_utf8_lossy(&bytes);
    for forbidden in [
        "amctl-dev-key",
        "lab-test-key",
        "@admin:",
        "@l0r3zz:",
        "@agent0-1",
        "@agent0-5",
        "v-site.net",
    ] {
        assert!(
            !text.contains(forbidden),
            "{forbidden} is compiled into {path}"
        );
    }
}
