//! Trusted-sender lookup for room invites.
//!
//! The API key is never logged. No Matrix user id is compiled into this module.

use std::collections::HashSet;
use std::time::Duration;

/// In-memory trusted sender ids. Empty until a refresh fills it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustMemory {
    ids: HashSet<String>,
}

impl TrustMemory {
    /// Start from an existing set. Used when a previous refresh already succeeded.
    #[allow(dead_code)]
    pub fn from_ids<I, S>(ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            ids: ids.into_iter().map(Into::into).collect(),
        }
    }

    /// Whether `sender` may invite this agent.
    pub fn admits(&self, sender: &str) -> bool {
        self.ids.contains(sender)
    }

    /// Current ids. A successful am-ctl refresh replaces this whole set.
    #[allow(dead_code)]
    pub fn ids(&self) -> &HashSet<String> {
        &self.ids
    }
}

/// What to do with one invite after the trusted set is current.
#[derive(Debug, PartialEq, Eq)]
pub enum InviteAction {
    Join,
    Reject,
}

/// `Join` only when the inviter is in the set.
pub fn invite_action(memory: &TrustMemory, sender: &str) -> InviteAction {
    if memory.admits(sender) {
        InviteAction::Join
    } else {
        InviteAction::Reject
    }
}

/// Greeting sent once after a trusted join.
pub fn greeting(display_name: &str) -> String {
    format!("Hello! I'm {display_name}, an AI agent powered by Agent Zero.")
}

/// Where to read trusted senders. `api_key` is absent when `AMCTL_API_KEY` is unset.
pub struct TrustQuery {
    pub amctl_url: String,
    pub api_key: Option<String>,
    pub fallback: Vec<String>,
}

/// Replace the set from am-ctl, or use `fallback` only while the set is empty.
///
/// A non-empty JSON array of objects with `matrix_user_id` replaces the set.
/// Any other outcome leaves a non-empty set unchanged.
pub async fn refresh(memory: &mut TrustMemory, query: &TrustQuery) {
    match fetch_ids(query).await {
        Some(ids) if !ids.is_empty() => {
            memory.ids = ids;
        }
        _ => {
            if memory.ids.is_empty() {
                memory.ids = query
                    .fallback
                    .iter()
                    .map(|id| id.trim())
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }
    }
}

async fn fetch_ids(query: &TrustQuery) -> Option<HashSet<String>> {
    let Some(api_key) = query.api_key.as_deref().filter(|key| !key.is_empty()) else {
        tracing::warn!("Trusted sender lookup failed: AMCTL_API_KEY is unset");
        return None;
    };
    let url = format!(
        "{}/api/v1/trusted-senders",
        query.amctl_url.trim_end_matches('/')
    );
    let client = matrix_sdk::reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let response = match client.get(url).header("X-API-Key", api_key).send().await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!("Trusted sender lookup failed");
            return None;
        }
    };
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(status = status.as_u16(), "Trusted sender lookup failed");
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    let items: Vec<serde_json::Value> = serde_json::from_slice(&bytes).ok()?;
    let mut ids = HashSet::new();
    for item in items {
        if let Some(user_id) = item
            .get("matrix_user_id")
            .and_then(serde_json::Value::as_str)
        {
            let user_id = user_id.trim();
            if !user_id.is_empty() {
                ids.insert(user_id.to_string());
            }
        }
    }
    Some(ids)
}
