use matrix_mcp_server_r2::matrix::ClientCache;

#[tokio::test]
async fn cache_set_and_get() {
    let cache = ClientCache::new();

    // Without a real Matrix client we just verify the cache mechanics
    // by checking that a missing key returns None
    let result = cache.get("@user:example.com", "https://matrix.example.com").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_shutdown_clears_all() {
    let cache = ClientCache::new();
    cache.shutdown_all().await;
    // No clients to clear, but should not panic
    let result = cache.get("@user:example.com", "https://matrix.example.com").await;
    assert!(result.is_none());
}
