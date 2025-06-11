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
