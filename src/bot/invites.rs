//! Pending-invite reconciliation.
//!
//! One pass reads `Client::invited_rooms` and joins a trusted room at most
//! once. The module does not open a crypto store and does not start a sync.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::dispatch::Runtime;
use super::trusted::{self, TrustMemory, TrustQuery};

/// What one pass decided for one room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The inviter is in the trusted set.
    Join,
    /// The inviter is not in the trusted set.
    SkipUntrusted,
    /// The inviter is missing or malformed.
    SkipNoInviter,
}

impl Decision {
    fn as_log_str(self) -> &'static str {
        match self {
            Decision::Join => "join",
            Decision::SkipUntrusted => "skip_untrusted",
            Decision::SkipNoInviter => "skip_no_inviter",
        }
    }
}

/// Whether a decision log line is new for this room.
pub fn should_log(previous: Option<Decision>, next: Decision) -> bool {
    previous != Some(next)
}

/// Compare the inviter by the full Matrix user id.
pub fn decide(inviter: Option<&UserId>, trust: &TrustMemory) -> Decision {
    let Some(inviter) = inviter else {
        return Decision::SkipNoInviter;
    };
    if trust.admits(inviter.as_str()) {
        Decision::Join
    } else {
        Decision::SkipUntrusted
    }
}

/// Whether a failed join can succeed without a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// A ban, a missing room, an unsupported room version, or another 4xx other than 429.
    Permanent,
    /// A network error, a timeout, a rate limit, a server error, or a 403 that says the user is not invited yet.
    Temporary,
}

impl std::fmt::Display for ErrorClass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            ErrorClass::Permanent => "permanent",
            ErrorClass::Temporary => "temporary",
        })
    }
}

fn join_error_suffix(status: &Option<u16>, code: &Option<String>) -> String {
    match (status, code.as_deref()) {
        (Some(status), Some(code)) => format!(" {status} {code}"),
        (Some(status), None) => format!(" {status}"),
        (None, Some(code)) => format!(" {code}"),
        (None, None) => String::new(),
    }
}

/// The last join failure. The display text never includes a server message body.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{class}{}", join_error_suffix(.status, .code))]
pub struct JoinError {
    /// Permanent or temporary.
    pub class: ErrorClass,
    /// HTTP status when the homeserver returned one.
    pub status: Option<u16>,
    /// Matrix error code, for example `M_FORBIDDEN`.
    pub code: Option<String>,
}

/// Per-room automatic join state.
#[derive(Clone, Debug, Default)]
pub struct InviteEntry {
    /// Inviter from `invite_details`. `None` means missing or malformed.
    pub inviter: Option<OwnedUserId>,
    /// The decision from the previous pass. Used to log only changes.
    pub last_decision: Option<Decision>,
    /// Join attempts made, from 0 to 5.
    pub attempts: u8,
    /// The last failed attempt.
    pub last_error: Option<JoinError>,
    /// When the next attempt may run. `None` means now, or never when terminal.
    pub next_attempt_at: Option<tokio::time::Instant>,
    /// True while this room's join request is running.
    pub in_flight: bool,
}

/// In-memory invite state shared by the reconciler and `list_invitations`.
pub struct InviteRegistry {
    inner: Mutex<HashMap<OwnedRoomId, InviteEntry>>,
    active: AtomicBool,
}

impl InviteRegistry {
    /// An empty registry. It stays inactive until the reconciler starts.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            active: AtomicBool::new(false),
        }
    }

    /// True after [`spawn_reconciler`] has started.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Drop entries for rooms that are no longer invited.
    pub async fn prune(&self, invited: &[OwnedRoomId]) {
        self.inner
            .lock()
            .await
            .retain(|room_id, _| invited.contains(room_id));
    }

    /// A copy of the current entries.
    pub async fn snapshot(&self) -> HashMap<OwnedRoomId, InviteEntry> {
        // The caller reads this map without holding the registry lock.
        self.inner.lock().await.clone()
    }
}

impl Default for InviteRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Load bot files and build the runtime one pass needs.
#[allow(dead_code)] // `tests/bot_invites.rs` builds a runtime without `attach`.
pub fn runtime_for(session: &super::Session) -> crate::error::Result<Arc<Runtime>> {
    let state = super::state::BotState::load(&session.state_dir)?;
    Ok(Arc::new(Runtime {
        // The handler context owns its own copy of the session strings.
        user_id: session.user_id.clone(),
        display_name: session.display_name.clone(),
        identity: session.identity.clone(),
        a0_api_url: session.a0_api_url.clone(),
        amctl_url: session.amctl_url.clone(),
        amctl_api_key: session.amctl_api_key.clone(),
        trusted_senders: session.trusted_senders.clone(),
        accepts_images: session.accepts_images,
        state: Mutex::new(state),
        trust: Mutex::new(TrustMemory::default()),
    }))
}

/// One pending invite prepared for `list_invitations`.
pub struct InvitationView {
    /// Room id.
    pub room_id: String,
    /// `m.room.name`, or the cached display name.
    pub name: Option<String>,
    /// Canonical alias.
    pub canonical_alias: Option<String>,
    /// Room topic.
    pub topic: Option<String>,
    /// Inviter. `None` when the member event is missing.
    pub inviter: Option<String>,
    /// `encrypted`, `unencrypted`, or `unknown`.
    pub encryption: String,
    /// `None` when the reconciler has not decided this room.
    pub decision: Option<Decision>,
    /// Join attempts so far.
    pub attempts: u8,
    /// `JoinError` display. Never a server message body.
    pub last_error: Option<String>,
}

/// Render the invite list. At most 100 rooms are included.
pub fn render_invitations(mut views: Vec<InvitationView>) -> serde_json::Value {
    views.sort_by(|left, right| left.room_id.cmp(&right.room_id));
    let truncated = views.len() > 100;
    if truncated {
        views.truncate(100);
    }
    let invitations: Vec<serde_json::Value> = views
        .into_iter()
        .map(|view| {
            let (trusted, inviter) = match view.decision {
                Some(Decision::Join) => ("true", view.inviter),
                Some(Decision::SkipUntrusted) => ("false", view.inviter),
                Some(Decision::SkipNoInviter) => ("false", None),
                None => ("unknown", view.inviter),
            };
            serde_json::json!({
                "roomId": view.room_id,
                "name": view.name,
                "canonicalAlias": view.canonical_alias,
                "topic": view.topic,
                "inviter": inviter,
                "encryption": view.encryption,
                "trusted": trusted,
                "attempts": view.attempts,
                "lastError": view.last_error,
            })
        })
        .collect();
    serde_json::json!({
        "count": invitations.len(),
        "truncated": truncated,
        "invitations": invitations,
    })
}

/// True when a 403 says the invite is not saved yet.
///
/// The message is read only for this check. It is not stored or logged.
fn not_invited_yet(error: &matrix_sdk::Error) -> bool {
    let Some(api) = error.as_client_api_error() else {
        return false;
    };
    let ruma::api::error::ErrorBody::Standard(body) = &api.body else {
        return false;
    };
    matches!(&body.kind, ruma::api::error::ErrorKind::Forbidden)
        && body.message.to_ascii_lowercase().contains("not invited")
}

/// Classify a failed `room.join()`. The server message body is discarded.
pub fn classify(error: &matrix_sdk::Error) -> JoinError {
    let Some(api) = error.as_client_api_error() else {
        return JoinError {
            class: ErrorClass::Temporary,
            status: None,
            code: None,
        };
    };
    let status = api.status_code.as_u16();
    let kind = error.client_api_error_kind();
    let code = kind.map(|kind| kind.errcode().to_string());
    let limited = status == 429
        || kind.is_some_and(|kind| matches!(kind, ruma::api::error::ErrorKind::LimitExceeded(_)));
    let named_permanent = kind.is_some_and(|kind| {
        matches!(
            kind,
            ruma::api::error::ErrorKind::Forbidden
                | ruma::api::error::ErrorKind::NotFound
                | ruma::api::error::ErrorKind::UnsupportedRoomVersion
        )
    });
    // A 403 of "not invited" can arrive before the homeserver saves the invite.
    let class = if not_invited_yet(error) || limited || (500..600).contains(&status) {
        ErrorClass::Temporary
    } else if named_permanent || (400..500).contains(&status) {
        ErrorClass::Permanent
    } else {
        ErrorClass::Temporary
    };
    JoinError {
        class,
        status: Some(status),
        code,
    }
}

/// How many times a join is retried, and how long the waits are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Wait after the first failure, before jitter.
    pub base: Duration,
    /// Longest wait, before jitter.
    pub cap: Duration,
    /// Attempts per room, including the first.
    pub max_attempts: u8,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(30),
            cap: Duration::from_secs(15 * 60),
            max_attempts: 5,
        }
    }
}

fn jitter_factor(room_id: &RoomId) -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos())
        .unwrap_or(0);
    let mut mixed = u64::from(nanos);
    for byte in room_id.as_bytes() {
        mixed = mixed.wrapping_mul(31).wrapping_add(u64::from(*byte));
    }
    let span = (mixed % 4001) as f64 / 10_000.0;
    0.8 + span
}

fn scale_duration(duration: Duration, factor: f64) -> Duration {
    let nanos = duration.as_secs_f64() * factor * 1_000_000_000.0;
    if !nanos.is_finite() || nanos <= 0.0 {
        return Duration::from_millis(1);
    }
    Duration::from_nanos(nanos.min(u64::MAX as f64) as u64)
}

/// Wait after failed attempt `n`. `n` starts at 1.
pub fn backoff(policy: &RetryPolicy, attempt: u32, room_id: &RoomId) -> Duration {
    let shift = attempt.saturating_sub(1).min(16);
    let grown = policy.base.saturating_mul(1_u32 << shift);
    let capped = if grown > policy.cap {
        policy.cap
    } else {
        grown
    };
    scale_duration(capped, jitter_factor(room_id))
}

fn join_is_due(entry: &InviteEntry, policy: &RetryPolicy, now: tokio::time::Instant) -> bool {
    if entry.in_flight || entry.attempts >= policy.max_attempts {
        return false;
    }
    if matches!(
        entry.last_error.as_ref().map(|error| error.class),
        Some(ErrorClass::Permanent)
    ) {
        return false;
    }
    match entry.next_attempt_at {
        Some(due) => due <= now,
        None => true,
    }
}

/// Decide and join the invited rooms once.
pub async fn reconcile_pass(
    client: &matrix_sdk::Client,
    runtime: &Arc<Runtime>,
    registry: &InviteRegistry,
    policy: &RetryPolicy,
) {
    let rooms = client.invited_rooms();
    let invited: Vec<OwnedRoomId> = rooms.iter().map(|room| room.room_id().to_owned()).collect();
    registry.prune(&invited).await;
    if rooms.is_empty() {
        return;
    }
    let query = TrustQuery {
        // One lookup owns these strings for this pass.
        amctl_url: runtime.amctl_url.clone(),
        api_key: runtime.amctl_api_key.clone(),
        fallback: runtime.trusted_senders.clone(),
    };
    let memory = {
        let mut memory = runtime.trust.lock().await;
        trusted::refresh(&mut memory, &query).await;
        // Every room in this pass uses the same trusted set.
        memory.clone()
    };
    for room in rooms {
        // The registry key and the later join share this id.
        let room_id = room.room_id().to_owned();
        let inviter = match room.invite_details().await {
            Ok(details) => Some(details.inviter_id),
            Err(_) => None,
        };
        let decision = decide(inviter.as_deref(), &memory);
        let start_join = {
            let mut map = registry.inner.lock().await;
            let entry = map.entry(room_id.clone()).or_default();
            // The entry keeps the inviter after this pass logs it.
            entry.inviter = inviter.clone();
            if should_log(entry.last_decision, decision) {
                entry.last_decision = Some(decision);
                let inviter_log = match &inviter {
                    Some(id) => id.to_string(),
                    None => "missing".to_string(),
                };
                tracing::info!(
                    room_id = %room_id,
                    inviter = %inviter_log,
                    decision = decision.as_log_str(),
                    "invite decision"
                );
            }
            let start = decision == Decision::Join
                && join_is_due(entry, policy, tokio::time::Instant::now());
            if start {
                entry.in_flight = true;
                entry.next_attempt_at = None;
            }
            start
        };
        if !start_join {
            continue;
        }
        match room.join().await {
            Ok(()) => {
                registry.inner.lock().await.remove(&room_id);
                let greeting = trusted::greeting(&runtime.display_name);
                let content =
                    ruma::events::room::message::RoomMessageEventContent::text_plain(greeting);
                if let Err(error) = room.send(content).await {
                    let _ = error;
                    tracing::warn!(room_id = %room_id, "Invite greeting was not sent");
                }
            }
            Err(error) => {
                let classified = classify(&error);
                let mut map = registry.inner.lock().await;
                if let Some(entry) = map.get_mut(&room_id) {
                    entry.in_flight = false;
                    entry.attempts = entry.attempts.saturating_add(1);
                    let retry = classified.class == ErrorClass::Temporary
                        && entry.attempts < policy.max_attempts;
                    let attempt = entry.attempts;
                    let error_class = classified.class;
                    entry.last_error = Some(classified);
                    entry.next_attempt_at = if retry {
                        Some(
                            tokio::time::Instant::now()
                                + backoff(policy, u32::from(attempt), &room_id),
                        )
                    } else {
                        None
                    };
                    tracing::warn!(
                        room_id = %room_id,
                        attempt,
                        error_class = %error_class,
                        "invite join failed"
                    );
                }
            }
        }
    }
}

async fn earliest_retry(registry: &InviteRegistry) -> Option<tokio::time::Instant> {
    registry
        .inner
        .lock()
        .await
        .values()
        .filter_map(|entry| entry.next_attempt_at)
        .min()
}

/// Run one pass now, then one pass after each sync or retry deadline.
pub fn spawn_reconciler(
    client: matrix_sdk::Client,
    runtime: Arc<Runtime>,
    registry: Arc<InviteRegistry>,
    notify: Arc<tokio::sync::Notify>,
    policy: RetryPolicy,
) -> JoinHandle<()> {
    registry.active.store(true, Ordering::Relaxed);
    tokio::spawn(async move {
        reconcile_pass(&client, &runtime, &registry, &policy).await;
        loop {
            let deadline = earliest_retry(&registry)
                .await
                .unwrap_or_else(|| tokio::time::Instant::now() + Duration::from_secs(3600));
            tokio::select! {
                _ = notify.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {}
            }
            reconcile_pass(&client, &runtime, &registry, &policy).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str) -> OwnedUserId {
        OwnedUserId::try_from(id).expect("fixture user id")
    }

    #[test]
    fn an_exact_full_user_id_joins() {
        let memory = TrustMemory::from_ids(["@agent0-5:example.com"]);
        let inviter = user("@agent0-5:example.com");
        assert_eq!(decide(Some(inviter.as_ref()), &memory), Decision::Join);
    }

    #[test]
    fn a_localpart_or_domain_match_does_not_join() {
        let memory = TrustMemory::from_ids(["@agent0-5:example.com"]);
        let localpart = user("@agent0-5:other.test");
        let domain = user("@other:example.com");
        assert_eq!(
            decide(Some(localpart.as_ref()), &memory),
            Decision::SkipUntrusted
        );
        assert_eq!(
            decide(Some(domain.as_ref()), &memory),
            Decision::SkipUntrusted
        );
    }

    #[test]
    fn a_missing_inviter_does_not_join() {
        let memory = TrustMemory::from_ids(["@agent0-5:example.com"]);
        assert_eq!(decide(None, &memory), Decision::SkipNoInviter);
    }

    #[test]
    fn an_empty_set_does_not_join() {
        let memory = TrustMemory::default();
        let inviter = user("@agent0-5:example.com");
        assert_eq!(
            decide(Some(inviter.as_ref()), &memory),
            Decision::SkipUntrusted
        );
    }

    #[test]
    fn a_decision_is_logged_only_when_it_changes() {
        assert!(should_log(None, Decision::Join));
        assert!(!should_log(Some(Decision::Join), Decision::Join));
        assert!(should_log(Some(Decision::SkipUntrusted), Decision::Join));
    }

    #[test]
    fn a_join_error_omits_missing_parts() {
        let error = JoinError {
            class: ErrorClass::Permanent,
            status: Some(403),
            code: Some("M_FORBIDDEN".to_string()),
        };
        assert_eq!(error.to_string(), "permanent 403 M_FORBIDDEN");
        let partial = JoinError {
            class: ErrorClass::Temporary,
            status: Some(502),
            code: None,
        };
        assert_eq!(partial.to_string(), "temporary 502");
    }

    fn view(room_id: &str, decision: Option<Decision>) -> InvitationView {
        InvitationView {
            room_id: room_id.to_string(),
            name: Some("Ops".to_string()),
            canonical_alias: Some("#ops:example.com".to_string()),
            topic: Some("Ops chat".to_string()),
            inviter: Some("@agent0-5:example.com".to_string()),
            encryption: "encrypted".to_string(),
            decision,
            attempts: 2,
            last_error: Some("temporary 502".to_string()),
        }
    }

    fn sdk_error(status: u16, kind: Option<ruma::api::error::ErrorKind>) -> matrix_sdk::Error {
        sdk_error_message(status, kind, "secret body")
    }

    fn sdk_error_message(
        status: u16,
        kind: Option<ruma::api::error::ErrorKind>,
        message: &str,
    ) -> matrix_sdk::Error {
        use ruma::api::client::uiaa::UiaaResponse;
        use ruma::api::error::{
            Error as RumaError, ErrorBody, FromHttpResponseError, StandardErrorBody,
        };
        let status_code = matrix_sdk::reqwest::StatusCode::from_u16(status).expect("status");
        let body = match kind {
            Some(kind) => ErrorBody::Standard(StandardErrorBody::new(kind, message.to_string())),
            None => ErrorBody::Json(serde_json::json!({"oops": true})),
        };
        let http = matrix_sdk::HttpError::Api(Box::new(FromHttpResponseError::Server(
            UiaaResponse::MatrixError(RumaError::new(status_code, body)),
        )));
        matrix_sdk::Error::from(http)
    }

    #[test]
    fn invitations_render_sorted_and_truncated() {
        let mut views = (0..101)
            .map(|index| {
                view(
                    &format!("!room{index:03}:example.com"),
                    Some(Decision::Join),
                )
            })
            .collect::<Vec<_>>();
        views.swap(0, 50);
        let rendered = render_invitations(views);
        assert_eq!(rendered["count"], 100);
        assert_eq!(rendered["truncated"], true);
        assert_eq!(rendered["invitations"][0]["roomId"], "!room000:example.com");
        assert_eq!(
            rendered["invitations"][99]["roomId"],
            "!room099:example.com"
        );
    }

    #[test]
    fn an_empty_invite_list_renders_an_empty_object() {
        let rendered = render_invitations(Vec::new());
        assert_eq!(
            rendered,
            serde_json::json!({"count": 0, "truncated": false, "invitations": []})
        );
    }

    #[test]
    fn trust_and_encryption_use_the_contract_strings() {
        let joined = render_invitations(vec![view("!b:example.com", Some(Decision::Join))]);
        assert_eq!(joined["invitations"][0]["trusted"], "true");
        assert_eq!(joined["invitations"][0]["encryption"], "encrypted");
        let untrusted = view("!a:example.com", Some(Decision::SkipUntrusted));
        let rendered = render_invitations(vec![untrusted]);
        assert_eq!(rendered["invitations"][0]["trusted"], "false");
        assert_eq!(
            rendered["invitations"][0]["inviter"],
            "@agent0-5:example.com"
        );
        let mut missing = view("!c:example.com", Some(Decision::SkipNoInviter));
        missing.encryption = "unencrypted".to_string();
        let rendered = render_invitations(vec![missing]);
        assert_eq!(rendered["invitations"][0]["trusted"], "false");
        assert!(rendered["invitations"][0]["inviter"].is_null());
        assert_eq!(rendered["invitations"][0]["encryption"], "unencrypted");
        let unknown = render_invitations(vec![view("!d:example.com", None)]);
        assert_eq!(unknown["invitations"][0]["trusted"], "unknown");
        let mut unknown_encryption = view("!e:example.com", None);
        unknown_encryption.encryption = "unknown".to_string();
        let rendered = render_invitations(vec![unknown_encryption]);
        assert_eq!(rendered["invitations"][0]["encryption"], "unknown");
    }

    #[test]
    fn classify_splits_permanent_and_temporary_errors() {
        use ruma::api::error::ErrorKind;
        for (status, kind) in [
            (403, Some(ErrorKind::Forbidden)),
            (404, Some(ErrorKind::NotFound)),
            (400, Some(ErrorKind::UnsupportedRoomVersion)),
            (400, Some(ErrorKind::BadJson)),
        ] {
            let error = classify(&sdk_error(status, kind));
            assert_eq!(error.class, ErrorClass::Permanent, "{status}");
            assert!(!error.to_string().contains("secret"));
        }
        assert_eq!(
            classify(&sdk_error(403, Some(ErrorKind::Forbidden))).to_string(),
            "permanent 403 M_FORBIDDEN"
        );
        let race = classify(&sdk_error_message(
            403,
            Some(ErrorKind::Forbidden),
            "Answer from v-site.net: [403 / M_FORBIDDEN] You are not invited to this room.",
        ));
        assert_eq!(race.class, ErrorClass::Temporary);
        assert_eq!(race.to_string(), "temporary 403 M_FORBIDDEN");
        assert!(!race.to_string().contains("invited"));
        let ban = classify(&sdk_error_message(
            403,
            Some(ErrorKind::Forbidden),
            "You are banned from this room.",
        ));
        assert_eq!(ban.class, ErrorClass::Permanent);
        for (status, kind) in [
            (429, Some(ErrorKind::LimitExceeded(Default::default()))),
            (502, None),
            (500, None),
        ] {
            assert_eq!(
                classify(&sdk_error(status, kind)).class,
                ErrorClass::Temporary,
                "{status}"
            );
        }
        let timeout = classify(&matrix_sdk::Error::Timeout);
        assert_eq!(timeout.class, ErrorClass::Temporary);
        assert!(timeout.status.is_none());
    }

    #[test]
    fn backoff_stays_within_twenty_percent_and_under_eighteen_minutes() {
        let policy = RetryPolicy::default();
        let room = ruma::OwnedRoomId::try_from("!ops:example.com").expect("room");
        let expected = [30_u64, 60, 120, 240];
        for (attempt, seconds) in expected.into_iter().enumerate() {
            let wait = backoff(&policy, (attempt as u32) + 1, &room);
            let low = Duration::from_secs_f64(seconds as f64 * 0.8);
            let high = Duration::from_secs_f64(seconds as f64 * 1.2);
            assert!(wait >= low && wait <= high, "{attempt} {wait:?}");
        }
        let large = backoff(&policy, 100, &room);
        assert!(large <= Duration::from_secs(18 * 60));
    }

    #[test]
    fn a_finished_room_is_not_due() {
        let policy = RetryPolicy::default();
        let now = tokio::time::Instant::now();
        let mut entry = InviteEntry {
            attempts: 5,
            ..InviteEntry::default()
        };
        assert!(!join_is_due(&entry, &policy, now));
        entry.attempts = 1;
        entry.last_error = Some(JoinError {
            class: ErrorClass::Permanent,
            status: Some(403),
            code: Some("M_FORBIDDEN".to_string()),
        });
        assert!(!join_is_due(&entry, &policy, now));
        entry.last_error = Some(JoinError {
            class: ErrorClass::Temporary,
            status: Some(502),
            code: None,
        });
        entry.next_attempt_at = Some(now + Duration::from_secs(30));
        assert!(!join_is_due(&entry, &policy, now));
    }

    #[tokio::test]
    async fn aborting_the_reconciler_finishes_within_one_second() {
        let client = matrix_sdk::Client::builder()
            .homeserver_url("http://127.0.0.1:9")
            .build()
            .await
            .expect("client");
        let dir = std::env::temp_dir().join(format!(
            "am-invite-abort-{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let runtime = runtime_for(&super::super::Session {
            user_id: "@agent0-5:example.com".to_string(),
            display_name: "Agent".to_string(),
            identity: "Agent Zero".to_string(),
            state_dir: dir,
            a0_api_url: "http://127.0.0.1:9/api".to_string(),
            amctl_url: "http://127.0.0.1:9".to_string(),
            amctl_api_key: None,
            trusted_senders: Vec::new(),
            accepts_images: false,
        })
        .expect("runtime");
        let handle = spawn_reconciler(
            client,
            runtime,
            Arc::new(InviteRegistry::new()),
            Arc::new(tokio::sync::Notify::new()),
            RetryPolicy::default(),
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
        let started = tokio::time::Instant::now();
        handle.abort();
        let _ = handle.await;
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
