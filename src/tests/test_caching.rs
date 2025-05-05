#[cfg(test)]
mod tests {
    use crate::cache::FastbuCache;
    use std::path::Path;

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let cache = FastbuCache::new();
        let key = "mykey";
        let value = "myvalue";

        // Set a value
        assert!(cache.set(key, value).await.is_ok());

        // Get the value
        let result = cache.get(key).await;
        assert_eq!(Some(value), result);
    }

    #[tokio::test]
    async fn test_disk_persistence() {
        let storage_path = Path::new("cache_storage");
        let key = "persisted_key";
        let value = "persisted_value";

        // Set a value
        assert!(cache.insert(key.to_string(), value.to_string()).await.is_ok());

        // Restart the cache (simulate server restart)
        let new_cache = FastbuCache::new();

        // Verify data is persisted
        let result = new_cache.get(key).await;
        assert_eq!(Some(value), result);
    }
}
