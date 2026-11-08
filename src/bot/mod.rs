//! Reactive Matrix bot dispatcher.
//!
//! This module does not construct a `matrix-sdk` client and does not open
//! `CRYPTO_STORE_PATH`. Message handlers are registered on the shared client
//! when `BOT_ENABLED=true`.

pub mod agent_zero;
mod dispatch;
mod state;
mod trigger;
pub mod trusted;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dispatch::Runtime;

/// Set when invite and message handlers are registered.
static HANDLERS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Whether the dispatcher handlers are registered.
pub fn handlers_registered() -> bool {
    HANDLERS_REGISTERED.load(Ordering::Relaxed)
}

/// Session values copied from [`crate::config::Config`] for one process.
pub struct Session {
    pub user_id: String,
    pub display_name: String,
    pub identity: String,
    pub state_dir: PathBuf,
    /// Agent Zero `POST` URL.
    pub a0_api_url: String,
    /// am-ctl base URL for `GET /api/v1/trusted-senders`.
    pub amctl_url: String,
    /// am-ctl API key. Absent when unset. Never logged.
    pub amctl_api_key: Option<String>,
    /// Fallback Matrix ids used only when the trusted set is empty.
    pub trusted_senders: Vec<String>,
}

/// Set a non-empty display name, then register handlers on the shared client.
///
/// Call this only when `BOT_ENABLED=true`, and call it before
/// `start_background_sync`. A homeserver rejection of the display name is
/// logged and does not stop the process. This does not open `CRYPTO_STORE_PATH`.
pub async fn attach(client: &matrix_sdk::Client, session: &Session) {
    let name = session.display_name.trim();
    if !name.is_empty() {
        if let Err(error) = client.account().set_display_name(Some(name)).await {
            tracing::warn!("Could not set bot display name: {error}");
        }
    }
    let state = match state::BotState::load(&session.state_dir) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(
                "Bot state could not be loaded; message handler not registered: {error}"
            );
            return;
        }
    };
    let ctx = Arc::new(Runtime {
        user_id: session.user_id.clone(),
        display_name: session.display_name.clone(),
        identity: session.identity.clone(),
        a0_api_url: session.a0_api_url.clone(),
        amctl_url: session.amctl_url.clone(),
        amctl_api_key: session.amctl_api_key.clone(),
        trusted_senders: session.trusted_senders.clone(),
        state: tokio::sync::Mutex::new(state),
        trust: tokio::sync::Mutex::new(trusted::TrustMemory::default()),
    });
    register_message_handler(client, Arc::clone(&ctx));
    register_invite_handler(client, Arc::clone(&ctx));
    #[cfg(feature = "e2ee")]
    register_encrypted_handler(client, ctx);
    HANDLERS_REGISTERED.store(true, Ordering::Relaxed);
}

fn register_message_handler(client: &matrix_sdk::Client, ctx: Arc<Runtime>) {
    use ruma::events::room::message::{MessageType, SyncRoomMessageEvent};

    client.add_event_handler(move |ev: SyncRoomMessageEvent, room: matrix_sdk::Room| {
        let ctx = Arc::clone(&ctx);
        async move {
            let incoming = match ev {
                SyncRoomMessageEvent::Original(ev) => {
                    let (msgtype, body) = match ev.content.msgtype {
                        MessageType::Text(text) => (Some("m.text".to_string()), Some(text.body)),
                        _ => (None, None),
                    };
                    OwnedIncoming {
                        event_id: ev.event_id.to_string(),
                        sender: ev.sender.to_string(),
                        origin_server_ts: millis(ev.origin_server_ts),
                        is_room_message: true,
                        msgtype,
                        body,
                        decrypt_error: None,
                    }
                }
                SyncRoomMessageEvent::Redacted(ev) => OwnedIncoming {
                    event_id: ev.event_id.to_string(),
                    sender: ev.sender.to_string(),
                    origin_server_ts: millis(ev.origin_server_ts),
                    is_room_message: true,
                    msgtype: None,
                    body: None,
                    decrypt_error: None,
                },
            };
            dispatch::handle(&room, &ctx, incoming.borrowed()).await;
        }
    });
}

fn register_invite_handler(client: &matrix_sdk::Client, ctx: Arc<Runtime>) {
    use ruma::events::room::member::{MembershipState, StrippedRoomMemberEvent};

    client.add_event_handler(move |ev: StrippedRoomMemberEvent, room: matrix_sdk::Room| {
        let ctx = Arc::clone(&ctx);
        async move {
            if ev.content.membership != MembershipState::Invite {
                return;
            }
            if ev.state_key.as_str() != ctx.user_id {
                return;
            }
            let sender = ev.sender.to_string();
            let room_id = room.room_id().to_string();
            let dedup_key = format!("invite:{room_id}:{sender}");
            if already_processed(&ctx, &dedup_key).await {
                return;
            }
            let query = trusted::TrustQuery {
                amctl_url: ctx.amctl_url.clone(),
                api_key: ctx.amctl_api_key.clone(),
                fallback: ctx.trusted_senders.clone(),
            };
            let action = {
                let mut memory = ctx.trust.lock().await;
                trusted::refresh(&mut memory, &query).await;
                trusted::invite_action(&memory, &sender)
            };
            match action {
                trusted::InviteAction::Reject => {
                    tracing::warn!(
                        room_id = %room_id,
                        sender = %sender,
                        "reject_untrusted"
                    );
                    record_processed(&ctx, &dedup_key).await;
                }
                trusted::InviteAction::Join => {
                    if let Err(error) = room.join().await {
                        tracing::warn!(room_id = %room_id, "Invite join failed: {error}");
                        return;
                    }
                    let greeting = trusted::greeting(&ctx.display_name);
                    let content =
                        ruma::events::room::message::RoomMessageEventContent::text_plain(greeting);
                    if let Err(error) = room.send(content).await {
                        tracing::warn!(room_id = %room_id, "Invite greeting was not sent: {error}");
                    }
                    record_processed(&ctx, &dedup_key).await;
                }
            }
        }
    });
}

async fn already_processed(ctx: &Runtime, event_id: &str) -> bool {
    ctx.state
        .lock()
        .await
        .processed_event_ids
        .iter()
        .any(|id| id == event_id)
}

async fn record_processed(ctx: &Runtime, event_id: &str) {
    let mut state = ctx.state.lock().await;
    trigger::push_processed(&mut state.processed_event_ids, event_id);
    if let Err(error) = state.save() {
        tracing::warn!("Could not save processed event ids: {error}");
    }
}

#[cfg(feature = "e2ee")]
fn register_encrypted_handler(client: &matrix_sdk::Client, ctx: Arc<Runtime>) {
    use matrix_sdk::event_handler::RawEvent;
    use ruma::events::room::encrypted::OriginalSyncRoomEncryptedEvent;

    client.add_event_handler(
        move |ev: OriginalSyncRoomEncryptedEvent, raw: RawEvent, room: matrix_sdk::Room| {
            let ctx = Arc::clone(&ctx);
            async move {
                let _ = ev;
                on_encrypted(raw, room, ctx).await;
            }
        },
    );
}

#[cfg(feature = "e2ee")]
async fn on_encrypted(
    raw: matrix_sdk::event_handler::RawEvent,
    room: matrix_sdk::Room,
    ctx: Arc<Runtime>,
) {
    use matrix_sdk::deserialized_responses::TimelineEventKind;
    use ruma::events::room::encrypted::OriginalSyncRoomEncryptedEvent;
    use ruma::serde::Raw;

    let value: serde_json::Value =
        serde_json::from_str(raw.get()).unwrap_or(serde_json::Value::Null);
    let event_id = json_string(&value, "event_id");
    let sender = json_string(&value, "sender");
    let origin_server_ts = value
        .get("origin_server_ts")
        .and_then(serde_json::Value::as_u64)
        .and_then(|millis| i64::try_from(millis).ok())
        .unwrap_or(0);
    let encrypted: Raw<OriginalSyncRoomEncryptedEvent> = Raw::from_json(raw.0);
    let incoming = match room.decrypt_event(&encrypted, None).await {
        Ok(timeline) => {
            if let TimelineEventKind::UnableToDecrypt { utd_info, .. } = &timeline.kind {
                OwnedIncoming {
                    event_id,
                    sender,
                    origin_server_ts,
                    is_room_message: false,
                    msgtype: None,
                    body: None,
                    decrypt_error: Some(utd_reason_token(&utd_info.reason).to_string()),
                }
            } else {
                owned_from_raw(timeline.raw(), event_id, sender, origin_server_ts)
            }
        }
        Err(error) => OwnedIncoming {
            event_id,
            sender,
            origin_server_ts,
            is_room_message: false,
            msgtype: None,
            body: None,
            decrypt_error: Some(error.to_string()),
        },
    };
    dispatch::handle(&room, &ctx, incoming.borrowed()).await;
}

#[cfg(feature = "e2ee")]
fn owned_from_raw(
    raw: &ruma::serde::Raw<ruma::events::AnySyncTimelineEvent>,
    fallback_event_id: String,
    fallback_sender: String,
    fallback_ts: i64,
) -> OwnedIncoming {
    let event_type = raw
        .get_field::<String>("type")
        .ok()
        .flatten()
        .unwrap_or_default();
    let event_id = raw
        .get_field::<String>("event_id")
        .ok()
        .flatten()
        .unwrap_or(fallback_event_id);
    let sender = raw
        .get_field::<String>("sender")
        .ok()
        .flatten()
        .unwrap_or(fallback_sender);
    let origin_server_ts = raw
        .get_field::<u64>("origin_server_ts")
        .ok()
        .flatten()
        .and_then(|millis| i64::try_from(millis).ok())
        .unwrap_or(fallback_ts);
    let content = raw.get_field::<serde_json::Value>("content").ok().flatten();
    let msgtype = content
        .as_ref()
        .and_then(|value| value.get("msgtype"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let body = content
        .as_ref()
        .and_then(|value| value.get("body"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    OwnedIncoming {
        event_id,
        sender,
        origin_server_ts,
        is_room_message: event_type == "m.room.message",
        msgtype,
        body,
        decrypt_error: None,
    }
}

#[cfg(feature = "e2ee")]
fn utd_reason_token(
    reason: &matrix_sdk::deserialized_responses::UnableToDecryptReason,
) -> &'static str {
    use matrix_sdk::deserialized_responses::UnableToDecryptReason;
    match reason {
        UnableToDecryptReason::MissingMegolmSession { .. } | UnableToDecryptReason::Unknown => {
            "missing_key"
        }
        UnableToDecryptReason::UnknownMegolmMessageIndex => "unknown_index",
        UnableToDecryptReason::MalformedEncryptedEvent => "malformed",
        UnableToDecryptReason::MegolmDecryptionFailure => "decryption_failure",
        UnableToDecryptReason::PayloadDeserializationFailure => "payload",
        UnableToDecryptReason::MismatchedIdentityKeys => "mismatched_identity",
        UnableToDecryptReason::SenderIdentityNotTrusted(_) => "sender_identity_not_trusted",
        #[allow(unreachable_patterns)]
        _ => "unknown",
    }
}

#[cfg(feature = "e2ee")]
fn json_string(value: &serde_json::Value, field: &str) -> String {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

struct OwnedIncoming {
    event_id: String,
    sender: String,
    origin_server_ts: i64,
    is_room_message: bool,
    msgtype: Option<String>,
    body: Option<String>,
    decrypt_error: Option<String>,
}

impl OwnedIncoming {
    fn borrowed(&self) -> trigger::Incoming<'_> {
        trigger::Incoming {
            event_id: &self.event_id,
            sender: &self.sender,
            origin_server_ts: self.origin_server_ts,
            is_room_message: self.is_room_message,
            msgtype: self.msgtype.as_deref(),
            body: self.body.as_deref(),
            decrypt_error: self.decrypt_error.as_deref(),
        }
    }
}

fn millis(ts: ruma::MilliSecondsSinceUnixEpoch) -> i64 {
    i64::from(ts.0)
}
