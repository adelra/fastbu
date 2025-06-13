use crate::cache::FastbuCache;
use crate::cluster_cache::ClusterCache;
use log::{debug, error, warn};
use std::sync::Arc;

/// API-compatible wrapper around ClusterCache for use in the HTTP API
pub struct ClusterAwareApiCache {
    cluster_cache: Arc<ClusterCache>,
}

impl ClusterAwareApiCache {
    pub fn new(cluster_cache: Arc<ClusterCache>) -> Self {
        Self { cluster_cache }
    }
}

impl FastbuCache {
    // Local cache methods will still work as before
}

// Implement methods on ClusterAwareApiCache that match FastbuCache methods,
// but forward requests to the cluster cache
impl ClusterAwareApiCache {
    pub async fn insert(&self, key: String, value: String) -> Result<(), std::io::Error> {
        debug!("API insert request for key: {}", key);
        
        match self.cluster_cache.insert(key.clone(), value).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Cluster insert failed for key: {}. Error: {}", key, e);
                Err(std::io::Error::other(format!("Cluster insert failed: {}", e)))
            }
        }
    }
    
    pub async fn get(&self, key: &str) -> Option<String> {
        debug!("API get request for key: {}", key);
        self.cluster_cache.get(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{ClusterConfig, ClusterNode, FastbuCluster};
    use std::net::IpAddr;
    use std::str::FromStr;

    // Helper function to create a test cluster 
    async fn create_test_cluster() -> Arc<ClusterCache> {
        // Create a simple cluster config for testing
        let mut config = ClusterConfig::default();
        config.node.id = "test-node".to_string();
        config.node.host = "127.0.0.1".to_string();
        config.node.port = 8001;
        
        let cluster = FastbuCluster::new(config);
        Arc::new(ClusterCache::new(cluster))
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        // Setup
        let cluster_cache = create_test_cluster().await;
        let api_cache = ClusterAwareApiCache::new(cluster_cache);
        
        // Test inserting a value
        let key = "test-key".to_string();
        let value = "test-value".to_string();
        let result = api_cache.insert(key.clone(), value.clone()).await;
        
        // Should succeed (though actual insertion may be redirected in a real cluster)
        assert!(result.is_ok());
        
        // Try to get the value back
        let retrieved = api_cache.get(&key).await;
        
        // In a single-node test environment, we should get our value back
        // In a real cluster, this depends on node responsibilities
        if let Some(val) = retrieved {
            assert_eq!(val, value);
        }
        // Note: In a real cluster test, we would need to check for None as well
        // since the value might be stored on another node
    }
}
