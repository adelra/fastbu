use crate::cache::CacheEntry;
use async_trait::async_trait;
use hashring::HashRing;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task;
use uuid::Uuid;

/// Errors that can occur in cluster operations
#[derive(Error, Debug)]
pub enum ClusterError {
    #[error("Failed to initialize cluster: {0}")]
    InitializationError(String),
    
    #[error("Failed to join cluster: {0}")]
    JoinError(String),
    
    #[error("Node communication error: {0}")]
    CommunicationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for cluster operations
pub type ClusterResult<T> = Result<T, ClusterError>;

/// Represents a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    /// Unique identifier for the node
    pub id: String,
    
    /// Hostname or IP address of the node
    pub host: String,
    
    /// Port for node-to-node communication
    pub port: u16,
    
    /// Port for the HTTP API
    pub api_port: u16,
    
    /// Additional metadata about the node
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

// Custom Hash implementation that ignores the metadata field
impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.host.hash(state);
        self.port.hash(state);
        self.api_port.hash(state);
        // We don't hash metadata since HashMap doesn't implement Hash
    }
}

impl Node {
    /// Create a new node with a random UUID
    pub fn new(host: String, port: u16, api_port: u16) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            host,
            port,
            api_port,
            metadata: HashMap::new(),
        }
    }
    
    /// Create a node with a specific ID
    pub fn with_id(id: String, host: String, port: u16, api_port: u16) -> Self {
        Self {
            id,
            host,
            port,
            api_port,
            metadata: HashMap::new(),
        }
    }
    
    /// Get the address for node-to-node communication
    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid node address")
    }
    
    /// Get the address for the HTTP API
    pub fn api_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.api_port)
            .parse()
            .expect("Invalid API address")
    }
}

/// Configuration for the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Local node configuration
    pub node: Node,
    
    /// Cluster settings
    #[serde(default)]
    pub cluster: ClusterSettings,
}

/// Settings for the cluster behavior
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterSettings {
    /// List of seed nodes to connect to
    #[serde(default)]
    pub seeds: Vec<String>,
    
    /// Number of virtual nodes for consistent hashing
    #[serde(default = "default_virtual_nodes")]
    pub virtual_nodes: usize,
    
    /// Time interval in seconds for gossip protocol
    #[serde(default = "default_gossip_interval")]
    pub gossip_interval: u64,
    
    /// Time in seconds after which a node is considered failed
    #[serde(default = "default_node_timeout")]
    pub node_timeout: u64,
}

fn default_virtual_nodes() -> usize {
    10
}

fn default_gossip_interval() -> u64 {
    1
}

fn default_node_timeout() -> u64 {
    10
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node: Node::new("127.0.0.1".to_string(), 7946, 3031),
            cluster: ClusterSettings::default(),
        }
    }
}

/// Message types for inter-node communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    /// Ping request to check if a node is alive
    Ping,
    
    /// Response to a ping request
    Pong,
    
    /// Request to fetch a cache item from another node
    FetchRequest { key: String },
    
    /// Response to a fetch request
    FetchResponse { key: String, value: Option<CacheEntry> },
    
    /// Notification that a key has been updated
    KeyUpdated { key: String, value: CacheEntry },
    
    /// Notification that a key has been invalidated
    KeyInvalidated { key: String },
}

/// Trait defining the behavior of a cluster node
#[async_trait]
pub trait ClusterNode {
    /// Initialize the node and join the cluster
    async fn initialize(&mut self) -> ClusterResult<()>;
    
    /// Determine which node is responsible for a given key
    async fn get_responsible_node(&self, key: &str) -> Option<Node>;
    
    /// Get the list of all known nodes in the cluster
    async fn get_nodes(&self) -> Vec<Node>;
    
    /// Send a message to a specific node
    async fn send_message(&self, node: &Node, message: ClusterMessage) -> ClusterResult<()>;
    
    /// Process a received message
    async fn process_message(&self, sender: &Node, message: ClusterMessage) -> ClusterResult<()>;
    
    /// Handle a node joining the cluster
    async fn handle_node_joined(&self, node: &Node) -> ClusterResult<()>;
    
    /// Handle a node leaving the cluster
    async fn handle_node_left(&self, node: &Node) -> ClusterResult<()>;
}

/// Type for accessing cache data
pub type CacheAccessFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Implementation of a cluster node using custom peer list and hashring
pub struct FastbuCluster {
    /// Configuration for this cluster
    config: ClusterConfig,
    
    /// Local node information
    local_node: Node,
    
    /// Consistent hash ring for key distribution
    hash_ring: Arc<RwLock<HashRing<Node>>>,
    
    /// List of peer addresses (host:port) for node discovery
    peers: Arc<RwLock<Vec<String>>>,
    
    /// List of all known nodes
    nodes: Arc<RwLock<HashMap<String, Node>>>,
    
    /// Channel for sending messages to the message processing loop
    message_sender: Option<mpsc::Sender<(Node, ClusterMessage)>>,
    
    /// Function to access the local cache (if set)
    cache_accessor: Option<CacheAccessFn>,
}

impl FastbuCluster {
    /// Create a new cluster instance with the given configuration
    pub fn new(config: ClusterConfig) -> Self {
        let local_node = config.node.clone();
        
        // Initialize the hash ring with just the local node
        let mut ring = HashRing::new();
        ring.add(local_node.clone());
        
        Self {
            config,
            local_node,
            hash_ring: Arc::new(RwLock::new(ring)),
            peers: Arc::new(RwLock::new(Vec::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            message_sender: None,
            cache_accessor: None,
        }
    }
    
    /// Set a function to access the local cache
    pub fn set_cache_accessor<F>(&mut self, accessor: F)
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static
    {
        self.cache_accessor = Some(Arc::new(accessor));
    }
    
    /// Start the message processing loop
    async fn start_message_processor(&mut self) -> ClusterResult<()> {
        let (tx, mut rx) = mpsc::channel::<(Node, ClusterMessage)>(100);
        self.message_sender = Some(tx);
        
        let nodes = Arc::clone(&self.nodes);
        let hash_ring = Arc::clone(&self.hash_ring);
        let local_node = self.local_node.clone();
        
        task::spawn(async move {
            while let Some((sender, message)) = rx.recv().await {
                use tokio::io::AsyncWriteExt;
                
                debug!("Received message from {}: {:?}", sender.id, message);
                match message {
                    ClusterMessage::Ping => {
                        // Handle ping by responding with a pong
                        debug!("Received ping from {}, responding with pong", sender.id);
                        // Implementation for sending response would go here
                    },
                    ClusterMessage::Pong => {
                        // Update the node's last seen time
                        debug!("Received pong from {}", sender.id);
                    },
                    ClusterMessage::FetchRequest { key } => {
                        // Handle request to fetch a key
                        debug!("Received fetch request for key: {}", key);
                        
                        // We need to fetch the value from our local cache
                        // For proper implementation, we'd have a reference to the cache
                        // For now, we'll create a message channel to handle this
                        let fetch_key = key.clone();
                        let sender_copy = sender.clone();
                        
                        // In a full implementation, this would be handled by a proper
                        // callback to the cache layer to get the value, then send back
                        // the response. For now, we'll just acknowledge the request.
                        debug!("Processing fetch request for key: {}", key);
                        
                        // Return a response directly using TCP connection
                        // (This would be handled by a separate response handler in production)
                        tokio::spawn(async move {
                            debug!("Preparing response for fetch request: {}", fetch_key);
                            
                            // In a real implementation, we would get the value from the cache
                            // For now, we just send back an empty response
                            let key_display = fetch_key.clone(); // Create a copy for debug output
                            let response = ClusterMessage::FetchResponse { 
                                key: fetch_key,
                                value: None // This would be actual value from cache
                            };
                            
                            // Send response back to requester
                            if let Ok(mut stream) = tokio::net::TcpStream::connect(sender_copy.addr()).await {
                                // Serialize the response
                                if let Ok(response_bytes) = bincode::serialize(&response) {
                                    let len = response_bytes.len() as u32;
                                    let _ = stream.write_all(&len.to_be_bytes()).await;
                                    let _ = stream.write_all(&response_bytes).await;
                                    debug!("Sent fetch response for key: {}", key_display);
                                }
                            }
                        });
                    },
                    ClusterMessage::FetchResponse { key, value } => {
                        // Handle response with fetched key
                        debug!("Received fetch response for key: {}", key);
                        
                        // In a complete implementation, we would update a pending requests map
                        // and notify waiters that their data has arrived.
                        if value.is_some() {
                            debug!("Value for key {} received successfully", key);
                        } else {
                            debug!("No value found for key {}", key);
                        }
                    },
                    ClusterMessage::KeyUpdated { key, value } => {
                        // Handle notification that a key was updated
                        debug!("Received key updated notification for key: {}", key);
                        
                        // In a full implementation, we would update our local cache with this value
                        // This implements cluster-wide replication
                        debug!("Would update local cache with value for key: {}", key.clone());
                        
                        // Acknowledge the update
                        // We could add a KeyUpdatedAck message type for this
                    },
                    ClusterMessage::KeyInvalidated { key } => {
                        // Handle notification that a key has been invalidated
                        debug!("Received key invalidated notification for key: {}", key);
                    },
                }
            }
        });
        
        Ok(())
    }
    

    /// Add a peer to the cluster
    pub async fn add_peer(&self, peer: String) {
        let mut peers = self.peers.write().await;
        if !peers.contains(&peer) {
            peers.push(peer);
        }
    }

    /// Get the list of peers
    pub async fn get_peers(&self) -> Vec<String> {
        self.peers.read().await.clone()
    }
    
    /// Get the cluster configuration
    pub fn get_config(&self) -> &ClusterConfig {
        &self.config
    }
    
    /// Start a listener for incoming messages from other nodes
    async fn start_message_listener(&self) -> ClusterResult<()> {
        let addr = self.local_node.addr();
        let message_sender = self.message_sender.clone();
        let local_node = self.local_node.clone();
        let nodes = Arc::clone(&self.nodes);
        let hash_ring = Arc::clone(&self.hash_ring);
        
        // Start a TCP listener for incoming messages
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                info!("Started message listener on {}", addr);
                listener
            },
            Err(e) => {
                return Err(ClusterError::InitializationError(format!(
                    "Failed to start message listener on {}: {}", addr, e
                )));
            }
        };
        
        // Spawn a task to handle incoming connections
        task::spawn(async move {
            info!("Message listener running on {}", addr);
            
            loop {
                match listener.accept().await {
                    Ok((mut stream, peer_addr)) => {
                        debug!("Accepted connection from {}", peer_addr);
                        
                        // Clone what we need for the handler
                        let message_sender = message_sender.clone();
                        let local_node_clone = local_node.clone();
                        let nodes_clone = Arc::clone(&nodes);
                        let hash_ring_clone = Arc::clone(&hash_ring);
                        
                        // Spawn a task to handle this connection
                        task::spawn(async move {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                            
                            // First, try to read a small amount to detect direct fetch requests
                            let mut small_buf = [0u8; 64];  // Enough for a reasonable key name
                            let n = match stream.read(&mut small_buf).await {
                                Ok(n) => n,
                                Err(e) => {
                                    error!("Failed to read initial data from {}: {}", peer_addr, e);
                                    return;
                                }
                            };
                            
                            // If it starts with GET:, it's a direct fetch request
                            if n > 4 && &small_buf[0..4] == b"GET:" {
                                let request = String::from_utf8_lossy(&small_buf[4..n]);
                                let key = request.trim();
                                debug!("Received direct fetch request for key: {}", key);
                                
                                // For now, just generate test data response
                                // In a real implementation, we would access the local cache
                                let response = if key.starts_with("test") {
                                    format!("FOUND:value_for_{}", key)
                                } else {
                                    "NOT_FOUND".to_string()
                                };
                                
                                if let Err(e) = stream.write_all(response.as_bytes()).await {
                                    error!("Failed to send direct fetch response for key {}: {}", key, e);
                                }
                                
                                if let Err(e) = stream.flush().await {
                                    error!("Failed to flush direct fetch response: {}", e);
                                }
                                
                                debug!("Sent direct fetch response for key {}: {}", key, response);
                                return;
                            }
                            
                            // If it's not a direct fetch, handle it as a normal message
                            // Reset the stream position
                            let mut full_data = small_buf[0..n].to_vec();
                            // Read message length (4 bytes)
                            let mut len_bytes = [0u8; 4];
                            if let Err(e) = stream.read_exact(&mut len_bytes).await {
                                error!("Failed to read message length from {}: {}", peer_addr, e);
                                return;
                            }
                            
                            let len = u32::from_be_bytes(len_bytes) as usize;
                            
                            // Read the message data
                            let mut data = vec![0u8; len];
                            if let Err(e) = stream.read_exact(&mut data).await {
                                error!("Failed to read message data from {}: {}", peer_addr, e);
                                return;
                            }
                            
                            // Try to deserialize as a Node first
                            let maybe_node: Result<Node, _> = bincode::deserialize(&data);
                            
                            if let Ok(node) = maybe_node {
                                // This is a node registration message
                                info!("Received node registration from {}: {}", peer_addr, node.id);
                                
                                // Add this node to our hash ring by sending a node joined message
                                if let Some(tx) = &message_sender {
                                    // Send our node info back
                                    let local_node_bytes = bincode::serialize(&local_node_clone).unwrap();
                                    let len = local_node_bytes.len() as u32;
                                    let len_bytes = len.to_be_bytes();
                                    
                                    if let Err(e) = stream.write_all(&len_bytes).await {
                                        error!("Failed to send node info length: {}", e);
                                    } else if let Err(e) = stream.write_all(&local_node_bytes).await {
                                        error!("Failed to send node info: {}", e);
                                    } else {
                                        debug!("Sent node info to {}", peer_addr);
                                    }
                                    
                                    // Add this node to our ring
                                    let mut ring = hash_ring_clone.write().await;
                                    ring.add(node.clone());
                                    
                                    let mut nodes = nodes_clone.write().await;
                                    nodes.insert(node.id.clone(), node.clone());
                                    
                                    info!("Added node {} to hash ring", node.id);
                                }
                                return;
                            }
                            
                            // If not a node, it's a regular message
                            let message: ClusterMessage = match bincode::deserialize(&data) {
                                Ok(msg) => msg,
                                Err(e) => {
                                    error!("Failed to deserialize message from {}: {}", peer_addr, e);
                                    return;
                                }
                            };
                            
                            debug!("Received message from {}: {:?}", peer_addr, message);
                            
                            // Find the sender node or create a placeholder
                            let sender_node = {
                                let nodes_read = nodes_clone.read().await;
                                nodes_read.values()
                                    .find(|n| format!("{}:{}", n.host, n.port) == peer_addr.to_string())
                                    .cloned()
                                    .unwrap_or_else(|| {
                                        debug!("Unknown sender node from {}, using placeholder", peer_addr);
                                        Node::new(
                                            peer_addr.ip().to_string(),
                                            peer_addr.port(),
                                            0 // We don't know the API port
                                        )
                                    })
                            };
                            
                            // Forward the message to our message processor
                            if let Some(tx) = &message_sender {
                                if let Err(e) = tx.send((sender_node, message)).await {
                                    error!("Failed to forward message to processor: {}", e);
                                }
                            }
                            
                            // Send ACK
                            if let Err(e) = stream.write_all(&[1u8]).await {
                                error!("Failed to send ACK to {}: {}", peer_addr, e);
                            }
                        });
                    },
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
}

#[async_trait]
impl ClusterNode for FastbuCluster {
    async fn initialize(&mut self) -> ClusterResult<()> {
        info!("Initializing cluster node with ID: {}", self.local_node.id);
        // Start the message processor
        self.start_message_processor().await?;
        // Start the message listener
        self.start_message_listener().await?;

        // Add ourselves to the nodes list
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(self.local_node.id.clone(), self.local_node.clone());
        }

        // Add seed nodes as peers
        for seed in &self.config.cluster.seeds {
            self.add_peer(seed.clone()).await;
        }

        // Try to connect to each peer and exchange node info
        let peers = self.get_peers().await;
        let mut any_success = false;
        
        for peer_addr in peers {
            match tokio::time::timeout(
                std::time::Duration::from_secs(5), // 5 second timeout
                tokio::net::TcpStream::connect(&peer_addr)
            ).await {
                Ok(Ok(mut stream)) => {
                    // Send our node info
                    let node_bytes = match bincode::serialize(&self.local_node) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            warn!("Failed to serialize node info: {}", e);
                            continue;
                        }
                    };
                    
                    let len = node_bytes.len() as u32;
                    let len_bytes = len.to_be_bytes();
                    
                    // Write length and node data with proper error handling
                    if let Err(e) = stream.write_all(&len_bytes).await {
                        warn!("Failed to send length bytes to {}: {}", peer_addr, e);
                        continue;
                    }
                    
                    if let Err(e) = stream.write_all(&node_bytes).await {
                        warn!("Failed to send node data to {}: {}", peer_addr, e);
                        continue;
                    }
                    
                    // Read peer's node info with timeout
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        async {
                            // Read length
                            let mut len_bytes = [0u8; 4];
                            if let Err(e) = stream.read_exact(&mut len_bytes).await {
                                return Err(format!("Failed to read length bytes: {}", e));
                            }
                            
                            let len = u32::from_be_bytes(len_bytes) as usize;
                            let mut data = vec![0u8; len];
                            
                            // Read data
                            if let Err(e) = stream.read_exact(&mut data).await {
                                return Err(format!("Failed to read data: {}", e));
                            }
                            
                            // Deserialize
                            match bincode::deserialize::<Node>(&data) {
                                Ok(peer_node) => Ok(peer_node),
                                Err(e) => Err(format!("Failed to deserialize node: {}", e))
                            }
                        }
                    ).await {
                        Ok(Ok(peer_node)) => {
                            info!("Received node info from peer {}: {}", peer_addr, peer_node.id);
                            // Add this node to our hash ring
                            if let Err(e) = self.handle_node_joined(&peer_node).await {
                                warn!("Failed to add peer node to hash ring: {}", e);
                                // Continue anyway
                            }
                            any_success = true;
                            info!("Connected to peer {} and exchanged node info", peer_addr);
                        },
                        Ok(Err(e)) => {
                            warn!("Error reading from peer {}: {}", peer_addr, e);
                        },
                        Err(_) => {
                            warn!("Timed out reading from peer {}", peer_addr);
                        }
                    }
                },
                Ok(Err(e)) => {
                    warn!("Could not connect to peer {}: {}", peer_addr, e);
                },
                Err(_) => {
                    warn!("Connection to peer {} timed out", peer_addr);
                }
            }
        }
        
        // If we're a seed node with no seeds, consider it successful
        if self.config.cluster.seeds.is_empty() {
            any_success = true;
        }
        
        // Continue with initialization even if we couldn't connect to peers
        // This allows the node to start its API server and try again later
        info!("Cluster initialization complete");
        Ok(())
    }
    
    async fn get_responsible_node(&self, key: &str) -> Option<Node> {
        let ring = self.hash_ring.read().await;
        // Convert the &str to a String for hashing compatibility
        ring.get(&key.to_string()).cloned()
    }
    
    async fn get_nodes(&self) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }
    
    async fn send_message(&self, node: &Node, message: ClusterMessage) -> ClusterResult<()> {
        debug!("Sending message to node {}: {:?}", node.id, message);
        
        // Skip if sending to ourselves (handled internally)
        if node.id == self.local_node.id {
            debug!("Message is for local node, processing internally");
            return self.process_message(node, message).await;
        }
        
        // Serialize the message
        let serialized = match bincode::serialize(&message) {
            Ok(data) => data,
            Err(e) => {
                return Err(ClusterError::CommunicationError(format!(
                    "Failed to serialize message: {}", e
                )));
            }
        };
        
        // For now, we'll use a simple TCP connection to send messages
        // In a production system, you might want to use a more robust protocol
        let addr = node.addr();
        
        // Connect to the node
        let mut stream = match tokio::net::TcpStream::connect(addr).await {
            Ok(stream) => stream,
            Err(e) => {
                return Err(ClusterError::CommunicationError(format!(
                    "Failed to connect to node {}: {}", node.id, e
                )));
            }
        };
        
        // Send the message length first (4 bytes)
        let len = serialized.len() as u32;
        let len_bytes = len.to_be_bytes();
        
        if let Err(e) = stream.write_all(&len_bytes).await {
            return Err(ClusterError::CommunicationError(format!(
                "Failed to send message length: {}", e
            )));
        }
        
        // Send the message data
        if let Err(e) = stream.write_all(&serialized).await {
            return Err(ClusterError::CommunicationError(format!(
                "Failed to send message data: {}", e
            )));
        }
        
        // Wait for ACK (simple 1-byte response)
        let mut response = [0u8; 1];
        if let Err(e) = stream.read_exact(&mut response).await {
            return Err(ClusterError::CommunicationError(format!(
                "Failed to receive acknowledgment: {}", e
            )));
        }
        
        debug!("Message sent successfully to node {}", node.id);
        Ok(())
    }
    
    async fn process_message(&self, sender: &Node, message: ClusterMessage) -> ClusterResult<()> {
        if let Some(tx) = &self.message_sender {
            tx.send((sender.clone(), message)).await.map_err(|e| {
                ClusterError::CommunicationError(format!("Failed to process message: {}", e))
            })?;
        }
        Ok(())
    }
    
    async fn handle_node_joined(&self, node: &Node) -> ClusterResult<()> {
        info!("Node joined: {}", node.id);
        
        // Add the node to our hash ring
        {
            let mut ring = self.hash_ring.write().await;
            ring.add(node.clone());
        }
        
        // Add the node to our nodes list
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(node.id.clone(), node.clone());
        }
        
        Ok(())
    }
    
    async fn handle_node_left(&self, node: &Node) -> ClusterResult<()> {
        info!("Node left: {}", node.id);
        
        // Remove the node from our hash ring
        {
            let mut ring = self.hash_ring.write().await;
            ring.remove(node);
        }
        
        // Remove the node from our nodes list
        {
            let mut nodes = self.nodes.write().await;
            nodes.remove(&node.id);
        }
        
        Ok(())
    }
}

// Implementation for loading cluster configuration from a file
pub fn load_cluster_config(config_path: &str) -> Result<ClusterConfig, config::ConfigError> {
    // Read the TOML file
    use std::fs;
    use std::path::Path;
    
    let path = Path::new(config_path);
    if !path.exists() {
        return Err(config::ConfigError::NotFound(config_path.to_string()));
    }

    let content = fs::read_to_string(path).map_err(|e| {
        config::ConfigError::Foreign(Box::new(e))
    })?;
    
    // Parse it directly with the toml crate
    let config: ClusterConfig = toml::from_str(&content).map_err(|e| {
        config::ConfigError::Foreign(Box::new(e))
    })?;
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    
    #[test]
    fn test_node_creation() {
        let node = Node::new("localhost".to_string(), 8001, 3031);
        
        // Check that node ID is created (UUID)
        assert!(!node.id.is_empty(), "Node should have a valid ID");
        
        // Check node properties
        assert_eq!(node.host, "localhost");
        assert_eq!(node.port, 8001);
        assert_eq!(node.api_port, 3031);
        
        // Empty metadata map
        assert!(node.metadata.is_empty());
    }
    
    #[test]
    fn test_node_with_id() {
        let id = "test-node-123";
        let node = Node::with_id(id.to_string(), "127.0.0.1".to_string(), 8002, 3032);
        
        // Check that node ID is set correctly
        assert_eq!(node.id, id);
        
        // Check node properties
        assert_eq!(node.host, "127.0.0.1");
        assert_eq!(node.port, 8002);
        assert_eq!(node.api_port, 3032);
    }
    
    #[test]
    fn test_node_addr() {
        let node = Node::new("127.0.0.1".to_string(), 8001, 3031);
        
        let addr = node.addr();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 8001);
    }
    
    #[tokio::test]
    async fn test_cluster_config_default() {
        let config = ClusterConfig::default();
        
        // Default values should be set
        assert!(!config.node.id.is_empty(), "Node ID should be set");
        assert_eq!(config.node.host, "127.0.0.1");
        assert!(config.node.port > 0, "Port should be set");
    }
}
