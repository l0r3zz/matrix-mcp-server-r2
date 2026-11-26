//! One Agent Zero turn. The API key is read from disk and is never logged.

use super::state::BotState;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const MAX_CHUNK_CHARS: usize = 4000;
const SPLIT_LOOKBACK: usize = 500;

/// A failed turn. Neither variant carries a reply body or the API key.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentZeroError {
    /// HTTP 401 after one key re-read and one retry.
    #[error("AgentZeroAuth: HTTP {status}")]
    AgentZeroAuth { status: u16 },
    /// The POST failed, timed out, or returned no answer.
    #[error("AgentZeroUnavailable")]
    AgentZeroUnavailable,
}

/// One image on an evaluator POST. `base64` is standard base64 with padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachment {
    /// `image-<safe-id>.<ext>`. The safe id uses ASCII letters, digits, `.`, `_`, and `-`.
    pub filename: String,
    /// Standard base64 of the image bytes. Not logged.
    pub base64: String,
}

/// Inputs for one question. `state_dir` holds `.a0-api-key` and `room_contexts.json`.
pub struct AgentZeroTurn {
    pub api_url: String,
    pub state_dir: std::path::PathBuf,
    pub room_id: String,
    pub sender: String,
    pub prompt: String,
    pub identity: String,
    pub user_id: String,
    /// Present only when one image is ready. Omitted from the JSON when absent.
    pub image: Option<ImageAttachment>,
}

/// Reply text from a successful POST, before HTML rendering.
pub struct AskSuccess {
    pub reply: String,
    pub context_id: String,
}

/// POST one turn, store a changed context id, and return the reply text.
///
/// A 401 re-reads `.a0-api-key` once and retries the POST once. An exact
/// `404 Context not found` for a saved context retries once with a fresh context.
/// Failure returns an error and does not invent a reply.
///
/// The process binary reaches the same POST through [`ask`]. This wrapper is
/// the integration-test entry that also updates `room_contexts.json`.
#[allow(dead_code)]
pub async fn complete_turn(turn: &AgentZeroTurn) -> Result<String, AgentZeroError> {
    let mut state =
        BotState::load(&turn.state_dir).map_err(|_| AgentZeroError::AgentZeroUnavailable)?;
    let context_id = state.context_id(&turn.room_id);
    let success = ask(turn, &context_id).await?;
    if state.remember_context(&turn.room_id, &success.context_id) {
        if let Err(error) = state.save() {
            tracing::warn!("Could not save room context: {error}");
        }
    }
    Ok(success.reply)
}

pub(crate) async fn ask(
    turn: &AgentZeroTurn,
    context_id: &str,
) -> Result<AskSuccess, AgentZeroError> {
    let Some(key) = read_key(&turn.state_dir) else {
        tracing::warn!(status = 401, "AgentZeroAuth");
        return Err(AgentZeroError::AgentZeroAuth { status: 401 });
    };
    let message = message_body(
        &turn.identity,
        &turn.user_id,
        &turn.sender,
        &turn.room_id,
        &turn.prompt,
    );
    let client = matrix_sdk::reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| AgentZeroError::AgentZeroUnavailable)?;
    let response = post(
        &client,
        &turn.api_url,
        &key,
        &message,
        context_id,
        turn.image.as_ref(),
    )
    .await?;
    if !context_id.is_empty() && response.status().as_u16() == 404 {
        if is_missing_context(response).await {
            tracing::warn!(
                room_id = %turn.room_id,
                "Agent Zero context not found; retrying with a fresh context"
            );
            let response = post(
                &client,
                &turn.api_url,
                &key,
                &message,
                "",
                turn.image.as_ref(),
            )
            .await?;
            return parse_success(response).await;
        }
        tracing::warn!(status = 404, "AgentZeroUnavailable");
        return Err(AgentZeroError::AgentZeroUnavailable);
    }
    if response.status().as_u16() == 401 {
        let Some(retried_key) = read_key(&turn.state_dir) else {
            tracing::warn!(status = 401, "AgentZeroAuth");
            return Err(AgentZeroError::AgentZeroAuth { status: 401 });
        };
        let response = post(
            &client,
            &turn.api_url,
            &retried_key,
            &message,
            context_id,
            turn.image.as_ref(),
        )
        .await?;
        if response.status().as_u16() == 401 {
            tracing::warn!(status = 401, "AgentZeroAuth");
            return Err(AgentZeroError::AgentZeroAuth { status: 401 });
        }
        return parse_success(response).await;
    }
    parse_success(response).await
}

/// Preface, sender, room id, then the triggered text.
pub fn message_body(
    identity: &str,
    user_id: &str,
    sender: &str,
    room_id: &str,
    prompt: &str,
) -> String {
    format!(
        "[System: You are {identity}, a sovereign AI agent on the Agent-Matrix network. Your Matrix ID is {user_id}. Always identify yourself as {identity}.]\n[MATRIX MESSAGE - reply with plain conversational text only, DO NOT use any Matrix tools, do not send messages via MCP. Keep responses concise and conversational - a few sentences max. No status reports or diagnostics unless explicitly asked.]\nFrom: {sender}\nRoom: {room_id}\n\n{prompt}"
    )
}

/// Plain-text chunks of at most 4000 characters. A newline in the last 500
/// characters of a full chunk is the split point.
pub fn chunk_plain(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let remaining = chars.len() - start;
        if remaining <= MAX_CHUNK_CHARS {
            chunks.push(chars[start..].iter().collect());
            break;
        }
        let window_start = start + MAX_CHUNK_CHARS - SPLIT_LOOKBACK;
        let window_end = start + MAX_CHUNK_CHARS;
        let split_at = chars[window_start..window_end]
            .iter()
            .rposition(|character| *character == '\n')
            .map(|offset| window_start + offset)
            .unwrap_or(window_end);
        let (end, next) = if split_at < window_end && chars[split_at] == '\n' {
            (split_at, split_at + 1)
        } else {
            (split_at, split_at)
        };
        if end <= start {
            chunks.push(chars[start..window_end].iter().collect());
            start = window_end;
            continue;
        }
        chunks.push(chars[start..end].iter().collect());
        start = next;
    }
    chunks
}

/// HTML for one plain-text chunk.
pub fn markdown_html(markdown: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(markdown, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// One `(plain, html)` pair per outbound Matrix message.
pub fn outbound_messages(reply: &str) -> Vec<(String, String)> {
    chunk_plain(reply)
        .into_iter()
        .map(|plain| {
            let html = markdown_html(&plain);
            (plain, html)
        })
        .collect()
}

fn read_key(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(".a0-api-key")).ok()?;
    let key = text.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

/// JSON body for one evaluator POST. `attachments` is omitted when `image` is absent.
pub(crate) fn request_body(
    message: &str,
    context_id: &str,
    image: Option<&ImageAttachment>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "message": message,
        "context_id": context_id,
    });
    if let Some(image) = image {
        body["attachments"] = serde_json::json!([
            {
                "filename": image.filename,
                "base64": image.base64,
            }
        ]);
    }
    body
}

/// One attachment for `bytes`. The filename is `image-<safe-id>.<ext>`.
pub(crate) fn image_attachment(
    event_id: &str,
    mimetype: Option<&str>,
    bytes: &[u8],
) -> ImageAttachment {
    ImageAttachment {
        filename: image_filename(event_id, mimetype),
        base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes),
    }
}

fn image_filename(event_id: &str, mimetype: Option<&str>) -> String {
    let mut safe = String::new();
    for character in event_id.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            safe.push(character);
        }
    }
    if safe.is_empty() {
        safe.push_str("event");
    }
    let ext = match mimetype.unwrap_or("").to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "img",
    };
    format!("image-{safe}.{ext}")
}

async fn post(
    client: &matrix_sdk::reqwest::Client,
    url: &str,
    key: &str,
    message: &str,
    context_id: &str,
    image: Option<&ImageAttachment>,
) -> Result<matrix_sdk::reqwest::Response, AgentZeroError> {
    let body = serde_json::to_vec(&request_body(message, context_id, image))
        .map_err(|_| AgentZeroError::AgentZeroUnavailable)?;
    client
        .post(url)
        .header("content-type", "application/json")
        .header("X-API-KEY", key)
        .body(body)
        .send()
        .await
        .map_err(|_| AgentZeroError::AgentZeroUnavailable)
}

async fn is_missing_context(response: matrix_sdk::reqwest::Response) -> bool {
    let Ok(bytes) = response.bytes().await else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|error| error.trim().eq_ignore_ascii_case("Context not found"))
}

async fn parse_success(
    response: matrix_sdk::reqwest::Response,
) -> Result<AskSuccess, AgentZeroError> {
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(status = status.as_u16(), "AgentZeroUnavailable");
        return Err(AgentZeroError::AgentZeroUnavailable);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| AgentZeroError::AgentZeroUnavailable)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| AgentZeroError::AgentZeroUnavailable)?;
    let reply = value
        .get("response")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if reply.is_empty() {
        tracing::warn!("AgentZeroUnavailable");
        return Err(AgentZeroError::AgentZeroUnavailable);
    }
    let context_id = value
        .get("context_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(AskSuccess { reply, context_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_reply_is_one_html_message() {
        let messages = outbound_messages("the *sky* is blue");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0, "the *sky* is blue");
        assert!(messages[0].1.contains("<em>sky</em>"));
    }

    #[test]
    fn a_newline_in_the_last_500_characters_is_the_split() {
        let head = "a".repeat(3600);
        let text = format!("{head}\n{}", "b".repeat(500));
        let chunks = chunk_plain(&text);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].ends_with('a'));
        assert!(chunks[1].starts_with('b'));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.chars().count() <= MAX_CHUNK_CHARS));
    }

    #[test]
    fn a_long_line_splits_at_4000_characters() {
        let text = "x".repeat(4001);
        let chunks = chunk_plain(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), 4000);
        assert_eq!(chunks[1].chars().count(), 1);
    }

    #[test]
    fn a_text_turn_omits_attachments() {
        let body = request_body("hello", "ctx-1", None);
        assert_eq!(body["message"], "hello");
        assert_eq!(body["context_id"], "ctx-1");
        assert!(body.get("attachments").is_none());
    }

    #[test]
    fn an_image_turn_has_one_attachment() {
        let image = image_attachment("$evt_1", Some("image/png"), b"png-bytes");
        assert_eq!(image.filename, "image-evt_1.png");
        assert_eq!(image.base64, "cG5nLWJ5dGVz");
        let message = message_body(
            "Agent Zero",
            "@agent0-5:example.com",
            "@other:example.com",
            "!room:example.com",
            "",
        );
        assert!(!message.is_empty());
        assert!(!message.contains(&image.base64));
        let body = request_body(&message, "", Some(&image));
        let attachments = body["attachments"].as_array().expect("attachments");
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0]["filename"], "image-evt_1.png");
        assert_eq!(attachments[0]["base64"], "cG5nLWJ5dGVz");
    }

    #[test]
    fn filename_extensions_and_the_safe_id_charset() {
        assert_eq!(
            image_attachment("$Ab:c.example.com", Some("image/jpeg"), b"a").filename,
            "image-Abc.example.com.jpeg"
        );
        assert_eq!(
            image_attachment("id", Some("image/webp"), b"a").filename,
            "image-id.webp"
        );
        assert_eq!(
            image_attachment("id", Some("image/gif"), b"a").filename,
            "image-id.gif"
        );
        assert_eq!(
            image_attachment("$$$", Some("text/plain"), b"a").filename,
            "image-event.img"
        );
        let filename = image_attachment("id", None, b"a").filename;
        assert!(filename.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        }));
    }
}
