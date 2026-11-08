//! Bot files outside the crypto store.
//!
//! `sync_cursor.json` is not read and not written. `matrix-sdk` owns the sync token.
//!
//! The binary crate compiles this module before later tasks call it, so unused
//! items are allowed here until the dispatcher reads them.

#![allow(dead_code)]

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Newest event ids stay at the end. Older ids drop off the front past this cap.
const MAX_PROCESSED_EVENT_IDS: usize = 1000;

/// Room context ids and the bounded processed-event list for one agent.
pub struct BotState {
    /// `room_id` to Agent Zero `context_id`.
    pub room_contexts: HashMap<String, String>,
    /// Event ids, newest at the end, length at most 1000.
    pub processed_event_ids: Vec<String>,
    dir: PathBuf,
}

impl BotState {
    /// Load state from `dir`. A missing JSON file is empty. A longer event list is trimmed from the front.
    pub fn load(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        let room_contexts: HashMap<String, String> =
            read_json_or_empty(&dir.join("room_contexts.json"))?;
        let mut processed_event_ids: Vec<String> =
            read_json_or_empty(&dir.join("processed_event_ids.json"))?;
        if processed_event_ids.len() > MAX_PROCESSED_EVENT_IDS {
            let drop_count = processed_event_ids.len() - MAX_PROCESSED_EVENT_IDS;
            processed_event_ids.drain(0..drop_count);
        }
        Ok(Self {
            room_contexts,
            processed_event_ids,
            dir,
        })
    }

    /// Context id for `room_id`. A missing entry is an empty string.
    pub fn context_id(&self, room_id: &str) -> String {
        self.room_contexts.get(room_id).cloned().unwrap_or_default()
    }

    /// Directory that holds the JSON files and `.a0-api-key`.
    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// Store a new context id for `room_id`. An empty id, or the current id, is left unchanged.
    pub fn remember_context(&mut self, room_id: &str, context_id: &str) -> bool {
        if context_id.is_empty() || self.context_id(room_id) == context_id {
            return false;
        }
        self.room_contexts
            .insert(room_id.to_string(), context_id.to_string());
        true
    }

    /// Write both JSON files. Does not create `sync_cursor.json`.
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        write_json(&self.dir.join("room_contexts.json"), &self.room_contexts)?;
        write_json(
            &self.dir.join("processed_event_ids.json"),
            &self.processed_event_ids,
        )?;
        Ok(())
    }
}

fn read_json_or_empty<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(AppError::from),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(AppError::from(error)),
    }
}

fn write_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("am-bot-state-{label}-{nanos}"));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn missing_files_are_empty_and_save_skips_sync_cursor() {
        let dir = temp_dir("missing");
        let mut state = BotState::load(&dir).expect("load missing");
        assert!(state.room_contexts.is_empty());
        assert!(state.processed_event_ids.is_empty());
        assert_eq!(state.context_id("!room:example"), "");
        state
            .room_contexts
            .insert("!room:example".to_string(), "ctx-1".to_string());
        state.save().expect("save");
        assert!(!dir.join("sync_cursor.json").exists());
        let loaded = BotState::load(&dir).expect("reload");
        assert_eq!(loaded.context_id("!room:example"), "ctx-1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn processed_ids_drop_from_the_front_past_1000() {
        let dir = temp_dir("cap");
        let ids: Vec<String> = (0..1005).map(|n| format!("$e{n}")).collect();
        fs::write(
            dir.join("processed_event_ids.json"),
            serde_json::to_vec(&ids).expect("json"),
        )
        .expect("write");
        let state = BotState::load(&dir).expect("load");
        assert_eq!(state.processed_event_ids.len(), 1000);
        assert_eq!(
            state.processed_event_ids.first().map(String::as_str),
            Some("$e5")
        );
        assert_eq!(
            state.processed_event_ids.last().map(String::as_str),
            Some("$e1004")
        );
        let _ = fs::remove_dir_all(dir);
    }
}
