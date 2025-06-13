use crate::cache::{CacheEntry, FastbuCache};
use crate::cluster::{ClusterNode, ClusterMessage, ClusterResult, FastbuCluster, Node};
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A cluster-aware cache that distributes data across nodes
pub struct ClusterCache {
    /// The local cache instance
    local_cache: FastbuCache,
    
    /// Reference to the cluster for node management
    cluster: Arc<RwLock<FastbuCluster>>,
}

impl ClusterCache {
    /// Create a new cluster-aware cache
    pub fn new(cluster: FastbuCluster) -> Self {
        // Create a new cache instance without the accessor yet
        let instance = Self {
            local_cache: FastbuCache::new(),
            cluster: Arc::new(RwLock::new(cluster)),
        };
        
        // We'll implement the cache accessor elsewhere since
        // we can't capture the local_cache directly due to lifetime constraints
        
        instance
    }
    
    /// Insert a key-value pair into the cache
    /// If this node is responsible for the key, store it locally
    /// Otherwise, forward the request to the responsible node
    pub async fn insert(&self, key: String, value: String) -> ClusterResult<()> {
        debug!("Cluster insert request for key: {}", key);

        // Find the node responsible for this key
        let cluster = self.cluster.read().await;
        let responsible_node = cluster.get_responsible_node(&key).await;

        match responsible_node {
            Some(node) => {
                // Get our local node information
                let local_id = cluster.get_config().node.id.clone();

                // Check if we are the responsible node
                if node.id == local_id {
                    debug!("This node is responsible for key: {}", key);
                    // We are responsible for this key, store it locally
                    match self.local_cache.insert(key.clone(), value.clone()).await {
                        Ok(_) => {
                            debug!("Successfully inserted key locally: {}", key);
                            // Notify other nodes that we've updated the key (for replication)
                            let entry = CacheEntry { value };
                            let message = ClusterMessage::KeyUpdated { key: key.clone(), value: entry };
                            
                            // Get all nodes in the cluster for replication
                            let nodes = cluster.get_nodes().await;
                            
                            // Send update to other nodes for redundancy (excluding ourselves)
                            for other_node in nodes.iter().filter(|n| n.id != local_id) {
                                debug!("Broadcasting key update to node: {}", other_node.id);
                                if let Err(e) = cluster.send_message(other_node, message.clone()).await {
                                    warn!("Failed to broadcast key update to node {}: {}", other_node.id, e);
                                }
                            }
                            
                            Ok(())
                        },
                        Err(e) => {
                            error!("Failed to insert key locally: {}. Error: {}", key, e);
                            Err(e.into())
                        }
                    }
                } else {
                    // Another node is responsible for this key, forward the request
                    debug!("Forwarding insert request for key: {} to node: {}", key, node.id);
                    // Create the message to send to the responsible node
                    let entry = CacheEntry { value: value.clone() };
                    let message = ClusterMessage::KeyUpdated { key: key.clone(), value: entry };
                    // Send the message to the responsible node
                    match cluster.send_message(&node, message).await {
                        Ok(_) => {
                            debug!("Successfully forwarded key {} to node {}", key, node.id);
                            return Ok(());
                        },
                        Err(e) => {
                            warn!("Failed to forward key {} to node {}: {}. Storing locally as fallback.", key, node.id, e);
                            match self.local_cache.insert(key.clone(), value).await {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e.into()),
                            }
                        }
                    }
                }
            },
            None => {
                // No responsible node found (should not happen in a properly configured cluster)
                warn!("No responsible node found for key: {}. Storing locally.", key);
                match self.local_cache.insert(key, value).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
    
    /// Get a value from the cache
    /// If this node is responsible for the key, get it locally
    /// Otherwise, fetch it from the responsible node
    pub async fn get(&self, key: &str) -> Option<String> {
        debug!("Cluster get request for key: {}", key);
        
        // Find the node responsible for this key
        let cluster = self.cluster.read().await;
        let responsible_node = cluster.get_responsible_node(key).await;
        
        match responsible_node {
            Some(node) => {
                // Get our local node information
                let local_id = cluster.get_config().node.id.clone();
                
                // Check if we are the responsible node
                if node.id == local_id {
                    debug!("This node is responsible for key: {}", key);
                    // We are responsible for this key, get it locally
                    self.local_cache.get(key)
                } else {
                    // Another node is responsible for this key
                    debug!("Fetching key: {} from responsible node: {}", key, node.id);
                    
                    // Send a fetch request to the responsible node
                    let fetch_message = ClusterMessage::FetchRequest { key: key.to_string() };
                    
                    match cluster.send_message(&node, fetch_message).await {
                        Ok(_) => {
                            debug!("Fetch request for key {} sent to node {}", key, node.id);
                            
                            // In a real implementation, we'd wait for a response
                            // For now, we'll simulate the response by checking locally first,
                            // and if not found, try a direct TCP connection to fetch the value
                            
                            // First check if we happen to have it locally (for faster response)
                            if let Some(value) = self.local_cache.get(key) {
                                debug!("Key {} found locally as fallback", key);
                                return Some(value);
                            }
                            
                            // Otherwise, try a direct fetch from the other node via TCP
                            debug!("Attempting direct fetch from node {}", node.id);
                            
                            // This would be implemented with a proper protocol
                            // For now, we'll use a simpler approach with a direct connection
                            let fetch_result = self.direct_fetch_from_node(&node, key).await;
                            
                            match fetch_result {
                                Some(value) => {
                                    debug!("Successfully fetched key {} from node {}", key, node.id);
                                    Some(value)
                                }
                                None => {
                                    warn!("Failed to fetch key {} from node {}", key, node.id);
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to send fetch request to node {}: {}", node.id, e);
                            // Fall back to local cache if communication fails
                            self.local_cache.get(key)
                        }
                    }
                }
            },
            None => {
                // No responsible node found (should not happen in a properly configured cluster)
                warn!("No responsible node found for key: {}. Checking locally.", key);
                self.local_cache.get(key)
            }
        }
    }
    
    /// Directly fetch a key value from another node using a TCP connection
    async fn direct_fetch_from_node(&self, node: &Node, key: &str) -> Option<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::time::timeout;
        use std::time::Duration;
        
        // Connect to the node's TCP address
        let addr = node.addr();
        info!("Attempting to fetch key '{}' directly from node {} at {}", key, node.id, addr);
        
        // Add a timeout for the connection to prevent blocking indefinitely
        let stream_result = timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(addr)).await;
        
        let mut stream = match stream_result {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                error!("Failed to connect to node {}: {}", node.id, e);
                return None;
            }
            Err(_) => {
                error!("Connection timeout when connecting to node {}", node.id);
                return None;
            }
        };
        
        // For the direct fetch approach, we'll actually implement a simpler protocol
        // that doesn't rely on the message handling code in the cluster

        // Send a simple direct fetch request: "GET:{key}"
        let request = format!("GET:{}", key);
        if let Err(e) = stream.write_all(request.as_bytes()).await {
            error!("Failed to send direct request to node {}: {}", node.id, e);
            return None;
        }
        
        // Flush to ensure the data is sent
        if let Err(e) = stream.flush().await {
            error!("Failed to flush request to node {}: {}", node.id, e);
            return None;
        }
        
        // Read the response with a timeout
        let mut response = String::new();
        match timeout(Duration::from_secs(5), stream.read_to_string(&mut response)).await {
            Ok(Ok(_)) => {
                debug!("Received response from node {}: {}", node.id, response);
                // Parse the response: FORMAT=FOUND:{value} or NOT_FOUND
                if response.starts_with("FOUND:") {
                    let value = response.strip_prefix("FOUND:").unwrap_or("").to_string();
                    info!("Successfully fetched key '{}' from node {}", key, node.id);
                    Some(value)
                } else if response == "NOT_FOUND" {
                    debug!("Key '{}' not found on node {}", key, node.id);
                    None
                } else {
                    error!("Invalid response from node {}: {}", node.id, response);
                    None
                }
            },
            Ok(Err(e)) => {
                error!("Failed to read response from node {}: {}", node.id, e);
                None
            },
            Err(_) => {
                error!("Response timeout when reading from node {}", node.id);
                None
            }
        }
    }
    
    /// Get a reference to the local cache
    pub fn local_cache(&self) -> &FastbuCache {
        &self.local_cache
    }
    
    /// Get the cluster reference
    pub fn cluster(&self) -> Arc<RwLock<FastbuCluster>> {
        self.cluster.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{ClusterConfig, ClusterNode};
    use std::net::IpAddr;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_cluster_cache_new() {
        // Create a simple cluster config for testing
        let mut config = ClusterConfig::default();
        config.node.id = "test-node".to_string();
        config.node.host = "127.0.0.1".to_string();
        config.node.port = 8001;
        
        let cluster = FastbuCluster::new(config);
        let cache = ClusterCache::new(cluster);
        
        // The test passes if ClusterCache::new doesn't panic
        assert!(true, "ClusterCache::new should not panic");
    }

    #[tokio::test]
    async fn test_local_operations() {
        // Create a simple cluster config for testing
        let mut config = ClusterConfig::default();
        config.node.id = "test-node".to_string();
        config.node.host = "127.0.0.1".to_string();
        config.node.port = 8001;
        
        let cluster = FastbuCluster::new(config);
        let cache = ClusterCache::new(cluster);
        
        // Since this is a single-node cluster, all operations should be local
        let key = "local-test-key".to_string();
        let value = "local-test-value".to_string();
        
        // Insert should succeed
        let result = cache.insert(key.clone(), value.clone()).await;
        assert!(result.is_ok(), "Local insert should succeed");
        
        // Get should return the inserted value
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some(), "Local get should find the key");
        assert_eq!(retrieved.unwrap(), value, "Retrieved value should match inserted value");
    }
}
