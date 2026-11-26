//! Apply a [`Decision`](crate::bot::trigger::Decision) on the shared client.
//!
//! Fast replies are sent here. An Agent Zero question is spawned off this task
//! and posted once from here.

use super::agent_zero::{self, AgentZeroError, AgentZeroTurn};
use super::state::BotState;
use super::trigger::{self, Decision, Incoming};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Values the message handler needs from process config.
pub struct Runtime {
    pub user_id: String,
    pub display_name: String,
    pub identity: String,
    pub a0_api_url: String,
    pub amctl_url: String,
    pub amctl_api_key: Option<String>,
    pub trusted_senders: Vec<String>,
    /// `true` only when `MODEL_ACCEPTS_IMAGES` is the string `true`.
    pub accepts_images: bool,
    pub state: Mutex<BotState>,
    pub trust: Mutex<super::trusted::TrustMemory>,
}

/// Classify, record, and send a fast reply. Agent Zero work runs on its own task.
pub async fn handle(room: &matrix_sdk::Room, ctx: &Arc<Runtime>, event: Incoming<'_>) {
    let prefixes = trigger::prefixes(&ctx.user_id, &ctx.display_name);
    let decision = {
        let mut state = ctx.state.lock().await;
        let decision = trigger::classify(
            &event,
            &ctx.user_id,
            &prefixes,
            &ctx.identity,
            &state.processed_event_ids,
            now_ms(),
        );
        if decision.record_event_id() {
            trigger::push_processed(&mut state.processed_event_ids, event.event_id);
            if let Err(error) = state.save() {
                tracing::warn!("Could not save processed event ids: {error}");
            }
        }
        decision
    };
    match decision {
        Decision::FastReply(body) => {
            let content = ruma::events::room::message::RoomMessageEventContent::text_plain(body);
            if let Err(error) = room.send(content).await {
                tracing::warn!(
                    room_id = %room.room_id(),
                    "Fast reply was not sent: {error}"
                );
            }
        }
        Decision::UnableToDecrypt { event_id, reason } => {
            tracing::warn!(
                event_id = %event_id,
                reason = %reason,
                "[Encrypted message - unable to decrypt ({reason})]"
            );
        }
        Decision::Dispatch(prompt) => {
            // The text task sends on this room after the handler returns.
            let room = room.clone();
            // The text task reads the runtime after the handler returns.
            let ctx = Arc::clone(ctx);
            let sender = event.sender.to_string();
            tokio::spawn(async move {
                answer_with_agent_zero(room, ctx, sender, prompt).await;
            });
        }
        Decision::ImagePrompt { caption } => {
            // The image task sends on this room after the handler returns.
            let room = room.clone();
            // The image task reads the runtime after the handler returns.
            let ctx = Arc::clone(ctx);
            let sender = event.sender.to_string();
            let event_id = event.event_id.to_string();
            let content = event.image_content.unwrap_or("").to_string();
            tokio::spawn(async move {
                answer_image(room, ctx, sender, event_id, caption, content).await;
            });
        }
        Decision::DropSelf
        | Decision::DropDuplicate
        | Decision::DropStale
        | Decision::DropNotText
        | Decision::NoTrigger => {}
    }
}

async fn answer_with_agent_zero(
    room: matrix_sdk::Room,
    ctx: Arc<Runtime>,
    sender: String,
    prompt: String,
) {
    set_typing(&room, true).await;
    let room_id = room.room_id().to_string();
    let (state_dir, context_id) = {
        let state = ctx.state.lock().await;
        (state.directory().to_path_buf(), state.context_id(&room_id))
    };
    let turn = AgentZeroTurn {
        api_url: ctx.a0_api_url.clone(),
        state_dir,
        room_id: room_id.clone(),
        sender,
        prompt,
        identity: ctx.identity.clone(),
        user_id: ctx.user_id.clone(),
        image: None,
    };
    let outcome = agent_zero::ask(&turn, &context_id).await;
    match outcome {
        Ok(success) => {
            {
                let mut state = ctx.state.lock().await;
                if state.remember_context(&room_id, &success.context_id) {
                    if let Err(error) = state.save() {
                        tracing::warn!("Could not save room context: {error}");
                    }
                }
            }
            send_reply(&room, &success.reply).await;
        }
        Err(AgentZeroError::AgentZeroAuth { .. } | AgentZeroError::AgentZeroUnavailable) => {}
    }
    set_typing(&room, false).await;
}

const ERR_CAPABILITY: &str = "This agent cannot evaluate an image. The prompt was not evaluated.";
const ERR_COUNT: &str = "Only one image is supported. The prompt was not evaluated.";
const ERR_SIZE: &str = "The image is too large. The prompt was not evaluated.";
const ERR_UNREADABLE: &str = "The image could not be read. The prompt was not evaluated.";
const ERR_NO_ANSWER: &str = "The model did not answer.";
const MAX_IMAGE_BYTES: u64 = 10_000_000;

/// Why an image prompt stops before the evaluator call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageStop {
    Count,
    Size,
    Unreadable,
}

/// Room text for a stop that happens before `ask`.
pub(crate) fn image_stop_text(stop: ImageStop) -> &'static str {
    match stop {
        ImageStop::Count => ERR_COUNT,
        ImageStop::Size => ERR_SIZE,
        ImageStop::Unreadable => ERR_UNREADABLE,
    }
}

/// Checks that can run before the media download.
pub(crate) fn image_stop_before_download(content: &serde_json::Value) -> Result<(), ImageStop> {
    let image: ruma::events::room::message::ImageMessageEventContent =
        serde_json::from_value(content.clone()).map_err(|_| ImageStop::Unreadable)?;
    if extra_media_urls(content, &image) {
        return Err(ImageStop::Count);
    }
    if let Some(size) = image.info.as_ref().and_then(|info| info.size) {
        if u64::from(size) > MAX_IMAGE_BYTES {
            return Err(ImageStop::Size);
        }
    }
    Ok(())
}

/// Checks on the bytes that came back from the download.
pub(crate) fn image_stop_after_download(
    content: &serde_json::Value,
    bytes: &[u8],
) -> Result<(), ImageStop> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(if bytes.len() as u64 > MAX_IMAGE_BYTES {
            ImageStop::Size
        } else {
            ImageStop::Unreadable
        });
    }
    if let Some(mimetype) = declared_mimetype(content) {
        if !mimetype.to_ascii_lowercase().starts_with("image/") {
            return Err(ImageStop::Unreadable);
        }
    }
    Ok(())
}

fn declared_mimetype(content: &serde_json::Value) -> Option<&str> {
    content
        .get("info")
        .and_then(|info| info.get("mimetype"))
        .and_then(serde_json::Value::as_str)
}

fn extra_media_urls(
    content: &serde_json::Value,
    image: &ruma::events::room::message::ImageMessageEventContent,
) -> bool {
    let primary = media_url(&image.source);
    let mut urls = Vec::new();
    collect_mxc(content, &mut urls);
    let thumbnails = thumbnail_urls(content);
    urls.into_iter()
        .any(|url| url != primary && !thumbnails.iter().any(|thumb| thumb == &url))
}

fn media_url(source: &ruma::events::room::MediaSource) -> String {
    match source {
        ruma::events::room::MediaSource::Plain(uri) => uri.to_string(),
        ruma::events::room::MediaSource::Encrypted(file) => file.url.to_string(),
    }
}

fn thumbnail_urls(content: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(info) = content.get("info") {
        if let Some(url) = info
            .get("thumbnail_url")
            .and_then(serde_json::Value::as_str)
        {
            urls.push(url.to_string());
        }
        if let Some(url) = info
            .get("thumbnail_file")
            .and_then(|file| file.get("url"))
            .and_then(serde_json::Value::as_str)
        {
            urls.push(url.to_string());
        }
    }
    urls
}

fn collect_mxc(value: &serde_json::Value, urls: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if text.starts_with("mxc://") => {
            urls.push(text.clone());
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_mxc(item, urls);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_mxc(item, urls);
            }
        }
        _ => {}
    }
}

async fn answer_image(
    room: matrix_sdk::Room,
    ctx: Arc<Runtime>,
    sender: String,
    event_id: String,
    caption: String,
    content: String,
) {
    let room_id = room.room_id().to_string();
    if !ctx.accepts_images {
        log_image(&room_id, &event_id, "refuse_capability");
        send_plain(&room, ERR_CAPABILITY).await;
        return;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        log_image(&room_id, &event_id, "refuse_unreadable");
        send_plain(&room, ERR_UNREADABLE).await;
        return;
    };
    if let Err(stop) = image_stop_before_download(&value) {
        log_image(&room_id, &event_id, stop_class(stop));
        send_plain(&room, image_stop_text(stop)).await;
        return;
    }
    let Ok(image) = serde_json::from_value::<ruma::events::room::message::ImageMessageEventContent>(
        value.clone(),
    ) else {
        log_image(&room_id, &event_id, "refuse_unreadable");
        send_plain(&room, ERR_UNREADABLE).await;
        return;
    };
    set_typing(&room, true).await;
    let downloaded = room
        .client()
        .media()
        .get_media_content(
            &matrix_sdk::media::MediaRequestParameters {
                // The media request stores its own source. The event content stays with this task.
                source: image.source.clone(),
                format: matrix_sdk::media::MediaFormat::File,
            },
            false,
        )
        .await;
    let bytes = match downloaded {
        Ok(bytes) => bytes,
        Err(_) => {
            log_image(&room_id, &event_id, "refuse_unreadable");
            send_plain(&room, ERR_UNREADABLE).await;
            set_typing(&room, false).await;
            return;
        }
    };
    if let Err(stop) = image_stop_after_download(&value, &bytes) {
        log_image(&room_id, &event_id, stop_class(stop));
        send_plain(&room, image_stop_text(stop)).await;
        drop(bytes);
        set_typing(&room, false).await;
        return;
    }
    let (state_dir, context_id) = {
        let state = ctx.state.lock().await;
        (state.directory().to_path_buf(), state.context_id(&room_id))
    };
    let turn = AgentZeroTurn {
        api_url: ctx.a0_api_url.clone(),
        state_dir,
        room_id: room_id.clone(),
        sender,
        prompt: caption,
        identity: ctx.identity.clone(),
        user_id: ctx.user_id.clone(),
        image: Some(agent_zero::image_attachment(
            &event_id,
            declared_mimetype(&value),
            &bytes,
        )),
    };
    drop(bytes);
    let outcome = agent_zero::ask(&turn, &context_id).await;
    match outcome {
        Ok(success) => {
            log_image(&room_id, &event_id, "sent");
            {
                let mut state = ctx.state.lock().await;
                if state.remember_context(&room_id, &success.context_id) {
                    if let Err(error) = state.save() {
                        tracing::warn!("Could not save room context: {error}");
                    }
                }
            }
            send_reply(&room, &success.reply).await;
        }
        Err(AgentZeroError::AgentZeroAuth { .. } | AgentZeroError::AgentZeroUnavailable) => {
            log_image(&room_id, &event_id, "evaluator_failed");
            send_plain(&room, ERR_NO_ANSWER).await;
        }
    }
    set_typing(&room, false).await;
}

fn stop_class(stop: ImageStop) -> &'static str {
    match stop {
        ImageStop::Count => "refuse_count",
        ImageStop::Size => "refuse_size",
        ImageStop::Unreadable => "refuse_unreadable",
    }
}

fn log_image(room_id: &str, event_id: &str, class: &'static str) {
    tracing::info!(room_id, event_id, class, "image prompt");
}

async fn send_plain(room: &matrix_sdk::Room, body: &str) {
    let content = ruma::events::room::message::RoomMessageEventContent::text_plain(body);
    if let Err(error) = room.send(content).await {
        tracing::warn!(
            room_id = %room.room_id(),
            "Image prompt error was not sent: {error}"
        );
    }
}

async fn send_reply(room: &matrix_sdk::Room, reply: &str) {
    for (plain, html) in agent_zero::outbound_messages(reply) {
        let content =
            ruma::events::room::message::RoomMessageEventContent::text_html(&plain, &html);
        if let Err(error) = room.send(content).await {
            tracing::warn!(
                room_id = %room.room_id(),
                "Agent Zero reply was not sent: {error}"
            );
            break;
        }
    }
}

async fn set_typing(room: &matrix_sdk::Room, typing: bool) {
    use ruma::api::client::typing::create_typing_event::v3::{Request, Typing, TypingInfo};
    let state = if typing {
        Typing::Yes(TypingInfo::new(Duration::from_millis(60_000)))
    } else {
        Typing::No
    };
    let request = Request::new(
        room.own_user_id().to_owned(),
        room.room_id().to_owned(),
        state,
    );
    if let Err(error) = room.client().send(request).await {
        tracing::warn!(
            room_id = %room.room_id(),
            "Typing notice was not updated: {error}"
        );
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(value: serde_json::Value) -> serde_json::Value {
        value
    }

    #[test]
    fn two_non_thumbnail_urls_are_more_than_one_image() {
        let value = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "extra": "mxc://example.com/two",
            "info": { "mimetype": "image/png", "size": 4 }
        }));
        assert_eq!(image_stop_before_download(&value), Err(ImageStop::Count));
    }

    #[test]
    fn a_thumbnail_url_does_not_count_as_a_second_image() {
        let value = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "info": {
                "mimetype": "image/png",
                "size": 4,
                "thumbnail_url": "mxc://example.com/thumb"
            }
        }));
        assert_eq!(image_stop_before_download(&value), Ok(()));
    }

    #[test]
    fn a_declared_size_over_the_limit_refuses_before_download() {
        let value = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "info": { "mimetype": "image/png", "size": 10_000_001 }
        }));
        assert_eq!(image_stop_before_download(&value), Err(ImageStop::Size));
    }

    #[test]
    fn a_downloaded_length_over_the_limit_uses_the_size_error() {
        let value = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "info": { "mimetype": "image/png" }
        }));
        let bytes = vec![0_u8; 10_000_001];
        assert_eq!(
            image_stop_after_download(&value, &bytes).map(|_| ()),
            Err(ImageStop::Size)
        );
        assert_eq!(
            image_stop_text(ImageStop::Size),
            "The image is too large. The prompt was not evaluated."
        );
    }

    #[test]
    fn empty_bytes_refuse() {
        let value = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one"
        }));
        assert_eq!(
            image_stop_after_download(&value, &[]),
            Err(ImageStop::Unreadable)
        );
    }

    #[test]
    fn a_declared_type_other_than_image_refuses_and_a_missing_type_does_not() {
        let other = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "info": { "mimetype": "application/pdf", "size": 4 }
        }));
        assert_eq!(
            image_stop_after_download(&other, b"abcd"),
            Err(ImageStop::Unreadable)
        );
        let missing = content(serde_json::json!({
            "body": "agent0-5: look",
            "url": "mxc://example.com/one",
            "info": { "size": 4 }
        }));
        assert_eq!(image_stop_after_download(&missing, b"abcd"), Ok(()));
    }
}
