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
            let room = room.clone();
            let ctx = Arc::clone(ctx);
            let sender = event.sender.to_string();
            tokio::spawn(async move {
                answer_with_agent_zero(room, ctx, sender, prompt).await;
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
