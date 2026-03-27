use matrix_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

const CLIENT_TTL: Duration = Duration::from_secs(15 * 60); // 15 minutes
const CLEANUP_INTERVAL: Duration = Duration::from_secs(5 * 60); // 5 minutes

#[derive(Clone)]
struct CachedClient {
    client: Client,
    created_at: Instant,
}

/// TTL-based client cache keyed by (user_id, homeserver_url).
/// Mirrors the TS `clientCache.ts` behavior.
#[derive(Clone)]
pub struct ClientCache {
    entries: Arc<RwLock<HashMap<String, CachedClient>>>,
}

impl ClientCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn cache_key(user_id: &str, homeserver_url: &str) -> String {
        format!("{}:{}", user_id, homeserver_url)
    }

    /// Get a cached client if it exists and hasn't expired.
    pub async fn get(&self, user_id: &str, homeserver_url: &str) -> Option<Client> {
        let key = Self::cache_key(user_id, homeserver_url);
        let entries = self.entries.read().await;
        entries.get(&key).and_then(|cached| {
            if cached.created_at.elapsed() < CLIENT_TTL {
                Some(cached.client.clone())
            } else {
                None
            }
        })
    }

    /// Store a client in the cache.
    pub async fn set(&self, user_id: &str, homeserver_url: &str, client: Client) {
        let key = Self::cache_key(user_id, homeserver_url);
        let mut entries = self.entries.write().await;
        entries.insert(
            key,
            CachedClient {
                client,
                created_at: Instant::now(),
            },
        );
    }

    /// Remove a client from the cache.
    pub async fn remove(&self, user_id: &str, homeserver_url: &str) {
        let key = Self::cache_key(user_id, homeserver_url);
        let mut entries = self.entries.write().await;
        entries.remove(&key);
    }

    /// Remove all expired entries.
    async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|_, cached| cached.created_at.elapsed() < CLIENT_TTL);
        let removed = before - entries.len();
        if removed > 0 {
            debug!("Cache cleanup: removed {} expired client(s)", removed);
        }
    }

    /// Shut down all cached clients.
    pub async fn shutdown_all(&self) {
        let mut entries = self.entries.write().await;
        let count = entries.len();
        entries.clear();
        info!("Shut down {} cached client(s)", count);
    }

    /// Start a periodic cleanup task. Returns a JoinHandle.
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
            loop {
                interval.tick().await;
                cache.cleanup().await;
            }
        })
    }
}
