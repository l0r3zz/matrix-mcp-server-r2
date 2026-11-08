//! Trigger prefixes and the event decision order.
//!
//! Prefixes come from the session Matrix user id. This module does not open a
//! socket and does not name a fleet instance.

/// Processed event ids kept in memory and on disk. Newest stay at the end.
pub const MAX_PROCESSED_EVENT_IDS: usize = 1000;

const STALE_AFTER_MS: i64 = 300_000;

/// One timeline event, already decrypted when decryption succeeded.
pub struct Incoming<'a> {
    pub event_id: &'a str,
    pub sender: &'a str,
    pub origin_server_ts: i64,
    /// `true` when the readable type is `m.room.message`.
    pub is_room_message: bool,
    pub msgtype: Option<&'a str>,
    pub body: Option<&'a str>,
    /// Set when decryption failed. The message body is not available.
    pub decrypt_error: Option<&'a str>,
}

/// What the dispatcher does with one event. The first match wins.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    DropSelf,
    DropDuplicate,
    DropStale,
    DropNotText,
    /// Decryption failed. Carries the event id and reason, and no message body.
    UnableToDecrypt {
        event_id: String,
        reason: String,
    },
    NoTrigger,
    FastReply(String),
    /// A trigger that is not a fast command. Phase 3 does not send it.
    Dispatch(String),
}

impl Decision {
    /// Reply body, when this decision sends one.
    #[cfg(test)]
    pub fn reply_body(&self) -> Option<&str> {
        match self {
            Decision::FastReply(body) => Some(body),
            Decision::DropSelf
            | Decision::DropDuplicate
            | Decision::DropStale
            | Decision::DropNotText
            | Decision::UnableToDecrypt { .. }
            | Decision::NoTrigger
            | Decision::Dispatch(_) => None,
        }
    }

    /// Duplicate ids are already stored. Every other decision is recorded.
    pub fn record_event_id(&self) -> bool {
        !matches!(self, Decision::DropDuplicate)
    }
}

/// Localpart of a Matrix user id, without the leading `@`.
pub fn agent_short(user_id: &str) -> String {
    user_id
        .split(':')
        .next()
        .unwrap_or(user_id)
        .trim_start_matches('@')
        .to_string()
}

/// Prefixes for `user_id`. A non-empty display name that differs from the
/// localpart adds three more. A prefix is required in every room.
pub fn prefixes(user_id: &str, display_name: &str) -> Vec<String> {
    let short = agent_short(user_id);
    let mut prefixes = vec![
        format!("{short}:"),
        format!("{short},"),
        format!("@{short}"),
        "@all-agents:".to_string(),
        "@all-agents,".to_string(),
    ];
    let display = display_name.trim();
    if !display.is_empty() && !display.eq_ignore_ascii_case(&short) {
        prefixes.push(format!("{display}:"));
        prefixes.push(format!("{display},"));
        prefixes.push(format!("@{display}"));
    }
    prefixes
}

/// Replace Element mention pills with the plain `@user:server` text.
pub fn strip_mention_pills(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut index = 0;
    while index < len {
        if chars[index] == '[' && index + 1 < len && chars[index + 1] == '@' {
            if let Some(relative_close) = chars[index..].iter().position(|c| *c == ']') {
                let bracket_close = index + relative_close;
                let after = bracket_close + 1;
                if after < len && chars[after] == '(' {
                    if let Some(relative_paren) = chars[after..].iter().position(|c| *c == ')') {
                        let paren_close = after + relative_paren;
                        let link: String = chars[after + 1..paren_close].iter().collect();
                        if link.starts_with("https://matrix.to/") {
                            let mention: String = chars[index + 1..bracket_close].iter().collect();
                            result.push_str(&mention);
                            index = paren_close + 1;
                            continue;
                        }
                    }
                }
            }
        }
        result.push(chars[index]);
        index += 1;
    }
    result
}

/// Remainder after the first matching prefix, trimmed. `None` when no prefix matches.
pub fn triggered_remainder(body: &str, prefixes: &[String]) -> Option<String> {
    let stripped = strip_mention_pills(body).trim().to_string();
    if stripped.is_empty() {
        return None;
    }
    let lower = stripped.to_lowercase();
    for prefix in prefixes {
        let prefix_lower = prefix.to_lowercase();
        if lower.starts_with(&prefix_lower) {
            let remainder: String = stripped.chars().skip(prefix.chars().count()).collect();
            return Some(remainder.trim().to_string());
        }
    }
    None
}

/// Append `event_id` and drop from the front past 1000. An empty id is ignored.
pub fn push_processed(ids: &mut Vec<String>, event_id: &str) {
    if event_id.is_empty() {
        return;
    }
    ids.push(event_id.to_string());
    if ids.len() > MAX_PROCESSED_EVENT_IDS {
        let drop_count = ids.len() - MAX_PROCESSED_EVENT_IDS;
        ids.drain(0..drop_count);
    }
}

/// Classify one event. `processed` is the list already stored, newest at the end.
pub fn classify(
    event: &Incoming<'_>,
    session_user_id: &str,
    prefixes: &[String],
    identity: &str,
    processed: &[String],
    now_ms: i64,
) -> Decision {
    if event.sender == session_user_id {
        return Decision::DropSelf;
    }
    if !event.event_id.is_empty() && processed.iter().any(|id| id == event.event_id) {
        return Decision::DropDuplicate;
    }
    if now_ms
        .checked_sub(event.origin_server_ts)
        .is_some_and(|age| age > STALE_AFTER_MS)
    {
        return Decision::DropStale;
    }
    if let Some(reason) = event.decrypt_error {
        return Decision::UnableToDecrypt {
            event_id: event.event_id.to_string(),
            reason: reason.to_string(),
        };
    }
    if !event.is_room_message || event.msgtype != Some("m.text") {
        return Decision::DropNotText;
    }
    let Some(remainder) = triggered_remainder(event.body.unwrap_or(""), prefixes) else {
        return Decision::NoTrigger;
    };
    if remainder.is_empty() {
        return Decision::NoTrigger;
    }
    if let Some(reply) = fast_reply(&remainder, identity, session_user_id) {
        return Decision::FastReply(reply);
    }
    Decision::Dispatch(remainder)
}

fn fast_reply(remainder: &str, identity: &str, user_id: &str) -> Option<String> {
    match remainder.trim().to_lowercase().as_str() {
        "ping" => Some("PONG".to_string()),
        "hello" => Some(format!("Hello! I'm {identity}, online and ready.")),
        "whoami" => Some(format!("I'm {identity} ({user_id}).")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &str = "@agent0-5:example.com";
    const OTHER: &str = "@other:example.com";
    const IDENTITY: &str = "Agent Zero";

    fn text<'a>(sender: &'a str, body: &'a str) -> Incoming<'a> {
        Incoming {
            event_id: "$evt",
            sender,
            origin_server_ts: 1_000_000,
            is_room_message: true,
            msgtype: Some("m.text"),
            body: Some(body),
            decrypt_error: None,
        }
    }

    #[test]
    fn prefixes_come_from_each_user_id() {
        let session = prefixes(SESSION, "");
        let other = prefixes(OTHER, "");
        assert!(session.iter().any(|prefix| prefix == "agent0-5:"));
        assert!(session.iter().any(|prefix| prefix == "@all-agents:"));
        assert!(other.iter().any(|prefix| prefix == "other:"));
        assert!(!other.iter().any(|prefix| prefix == "agent0-5:"));
        assert_eq!(prefixes(SESSION, "agent0-5").len(), session.len());
        assert!(prefixes(SESSION, "Galadriel")
            .iter()
            .any(|prefix| prefix == "Galadriel:"));
    }

    #[test]
    fn mention_pills_become_plain_mentions() {
        let pill = "[@agent0-5:example.com](https://matrix.to/#/@agent0-5:example.com)";
        assert_eq!(strip_mention_pills(pill), "@agent0-5:example.com");
        let body = format!("{pill} ping");
        let remainder = triggered_remainder(&body, &prefixes(SESSION, "")).expect("prefix");
        assert!(remainder.contains("ping"));
    }

    #[test]
    fn prefix_match_is_case_insensitive() {
        let decision = classify(
            &text("@other:example.com", "AGENT0-5: PiNg"),
            SESSION,
            &prefixes(SESSION, ""),
            IDENTITY,
            &[],
            1_000_000,
        );
        assert_eq!(decision, Decision::FastReply("PONG".to_string()));
        assert_eq!(
            classify(
                &text("@other:example.com", "agent0-5: hello"),
                SESSION,
                &prefixes(SESSION, ""),
                IDENTITY,
                &[],
                1_000_000,
            ),
            Decision::FastReply("Hello! I'm Agent Zero, online and ready.".to_string())
        );
        assert_eq!(
            classify(
                &text("@other:example.com", "@all-agents, whoami"),
                SESSION,
                &prefixes(SESSION, ""),
                IDENTITY,
                &[],
                1_000_000,
            ),
            Decision::FastReply(format!("I'm {IDENTITY} ({SESSION})."))
        );
    }

    #[test]
    fn processed_ids_drop_from_the_front_past_1000() {
        let mut ids: Vec<String> = (0..1000).map(|n| format!("$e{n}")).collect();
        push_processed(&mut ids, "$e1000");
        assert_eq!(ids.len(), 1000);
        assert_eq!(ids.first().map(String::as_str), Some("$e1"));
        assert_eq!(ids.last().map(String::as_str), Some("$e1000"));
    }

    #[test]
    fn stale_event_is_dropped_past_300000_ms() {
        let event = text("@other:example.com", "agent0-5: ping");
        assert_eq!(
            classify(
                &event,
                SESSION,
                &prefixes(SESSION, ""),
                IDENTITY,
                &[],
                1_300_000
            ),
            Decision::FastReply("PONG".to_string())
        );
        let stale = Incoming {
            origin_server_ts: 1_000_000,
            ..event
        };
        assert_eq!(
            classify(
                &stale,
                SESSION,
                &prefixes(SESSION, ""),
                IDENTITY,
                &[],
                1_000_000 + 300_001
            ),
            Decision::DropStale
        );
    }

    #[test]
    fn self_sender_is_dropped() {
        let decision = classify(
            &text(SESSION, "agent0-5: ping"),
            SESSION,
            &prefixes(SESSION, ""),
            IDENTITY,
            &[],
            1_000_000,
        );
        assert_eq!(decision, Decision::DropSelf);
        assert!(decision.record_event_id());
        assert!(decision.reply_body().is_none());
    }

    #[test]
    fn non_text_is_dropped() {
        let event = Incoming {
            msgtype: Some("m.notice"),
            ..text("@other:example.com", "agent0-5: ping")
        };
        assert_eq!(
            classify(
                &event,
                SESSION,
                &prefixes(SESSION, ""),
                IDENTITY,
                &[],
                1_000_000
            ),
            Decision::DropNotText
        );
    }

    #[test]
    fn unable_to_decrypt_has_event_id_and_reason_and_no_body() {
        let event = Incoming {
            event_id: "$secret",
            sender: "@other:example.com",
            origin_server_ts: 1_000_000,
            is_room_message: false,
            msgtype: None,
            body: None,
            decrypt_error: Some("missing_key"),
        };
        let decision = classify(
            &event,
            SESSION,
            &prefixes(SESSION, ""),
            IDENTITY,
            &[],
            1_000_000,
        );
        match &decision {
            Decision::UnableToDecrypt { event_id, reason } => {
                assert_eq!(event_id, "$secret");
                assert_eq!(reason, "missing_key");
            }
            other => panic!("expected unable to decrypt, got {other:?}"),
        }
        assert!(decision.reply_body().is_none());
    }

    #[test]
    fn bare_ping_and_other_users_prefix_are_not_triggers() {
        let session_prefixes = prefixes(SESSION, "");
        assert_eq!(
            classify(
                &text("@other:example.com", "ping"),
                SESSION,
                &session_prefixes,
                IDENTITY,
                &[],
                1_000_000,
            ),
            Decision::NoTrigger
        );
        assert_eq!(
            classify(
                &text("@other:example.com", "agent0-5: what is the time"),
                SESSION,
                &session_prefixes,
                IDENTITY,
                &["$evt".to_string()],
                1_000_000,
            ),
            Decision::DropDuplicate
        );
        assert_eq!(
            classify(
                &text("@agent0-5:example.com", "other: ping"),
                OTHER,
                &prefixes(OTHER, ""),
                IDENTITY,
                &[],
                1_000_000,
            ),
            Decision::FastReply("PONG".to_string())
        );
    }

    #[test]
    fn a_question_is_recognized_and_has_no_reply_body() {
        let decision = classify(
            &text("@other:example.com", "agent0-5: what is the time"),
            SESSION,
            &prefixes(SESSION, ""),
            IDENTITY,
            &[],
            1_000_000,
        );
        assert_eq!(decision, Decision::Dispatch("what is the time".to_string()));
        assert!(decision.reply_body().is_none());
    }
}
