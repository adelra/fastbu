use async_trait::async_trait;
use std::io;

/// Trait for cache implementations that can be used with the API
#[async_trait]
pub trait ApiCache: Send + Sync {
    /// Get a value from the cache
    async fn get(&self, key: &str) -> Option<String>;
    
    /// Set a value in the cache
    async fn set(&self, key: String, value: String) -> Result<(), io::Error>;
}

/// Implement the ApiCache trait for FastbuCache
#[async_trait]
impl ApiCache for crate::cache::FastbuCache {
    async fn get(&self, key: &str) -> Option<String> {
        self.get(key)
    }
    
    async fn set(&self, key: String, value: String) -> Result<(), io::Error> {
        self.insert(key, value).await
    }
}

/// Implement the ApiCache trait for ClusterAwareApiCache
#[async_trait]
impl ApiCache for crate::api_cache::ClusterAwareApiCache {
    async fn get(&self, key: &str) -> Option<String> {
        self.get(key).await
    }
    
    async fn set(&self, key: String, value: String) -> Result<(), io::Error> {
        self.insert(key, value).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::FastbuCache;

    #[tokio::test]
    async fn test_fastbu_cache_impl() {
        let cache = FastbuCache::new();

        // Test setting a value
        let key = "test_key".to_string();
        let value = "test_value".to_string();
        let set_result = <FastbuCache as ApiCache>::set(&cache, key.clone(), value.clone()).await;
        assert!(set_result.is_ok());

        // Test getting the value
        let get_result = <FastbuCache as ApiCache>::get(&cache, &key).await;
        assert!(get_result.is_some());
        assert_eq!(get_result.unwrap(), value);
    }

    #[tokio::test]
    async fn test_get_nonexistent_key() {
        let cache = FastbuCache::new();

        // Test getting a nonexistent key
        let get_result = <FastbuCache as ApiCache>::get(&cache, "nonexistent_key").await;
        assert!(get_result.is_none());
    }
}
