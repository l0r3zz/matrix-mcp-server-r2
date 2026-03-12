use crate::error::Result;
use matrix_sdk::room::Room;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache for Matrix data to reduce API calls
#[derive(Default)]
pub struct ClientCache {
    rooms: RwLock<HashMap<String, CachedRoom>>,
    users: RwLock<HashMap<String, CachedUser>>,
}

#[derive(Clone)]
pub struct CachedRoom {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub cached_at: std::time::Instant,
}

#[derive(Clone)]
pub struct CachedUser {
    pub user_id: String,
    pub display_name: Option<String>,
    pub cached_at: std::time::Instant,
}

impl ClientCache {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub async fn get_room(&self, room_id: &str) -> Option<CachedRoom> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }
    
    pub async fn set_room(&self, room: CachedRoom) {
        let mut rooms = self.rooms.write().await;
        rooms.insert(room.room_id.clone(), room);
    }
}
