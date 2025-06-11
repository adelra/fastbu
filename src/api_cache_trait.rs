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
