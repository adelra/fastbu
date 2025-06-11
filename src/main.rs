mod api;
mod api_cache; // Add the new api_cache module
mod api_cache_trait; // Add the trait for API caches
mod cache;
mod cluster; // Add the new cluster module
mod cluster_cache; // Add the new cluster_cache module
mod storage;

use crate::cache::FastbuCache;
use crate::cluster::{ClusterConfig, ClusterNode, FastbuCluster, load_cluster_config}; // Import cluster types
use crate::cluster_cache::ClusterCache; // Import cluster cache
use crate::api_cache::ClusterAwareApiCache; // Import API cache wrapper
use crate::api_cache_trait::ApiCache; // Import API cache trait
use clap::Parser;
use env_logger::Builder;
use log::{info, warn, LevelFilter};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3031;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 3031)]
    port: u16,
    
    /// Run in cluster mode
    #[arg(long)]
    cluster: bool,
    
    /// Path to cluster configuration file
    #[arg(long, default_value = "cluster.toml")]
    cluster_config: PathBuf,
    
    /// Node ID (overrides config file)
    #[arg(long)]
    node_id: Option<String>,
}

#[derive(Debug)]
struct Config {
    host: String,
    port: u16,
    cluster_mode: bool,
    cluster_config_path: PathBuf,
    node_id: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            cluster_mode: false,
            cluster_config_path: PathBuf::from("cluster.toml"),
            node_id: None,
        }
    }
}

fn setup_logging() {
    Builder::new()
        .filter_level(LevelFilter::Debug) // Change from Info to Debug
        .parse_filters(&std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string())) // Respect RUST_LOG
        .format_timestamp(None)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Setup logging
    setup_logging();
    info!("Starting Fastbu cache server...");

    // Parse command-line arguments
    let args = Args::parse();
    
    let config = Config {
        host: args.host.clone(),
        port: args.port,
        cluster_mode: args.cluster,
        cluster_config_path: args.cluster_config.clone(),
        node_id: args.node_id.clone(),
    };

    info!("Server configuration:");
    info!("Host: {}", config.host);
    info!("Port: {}", config.port);
    
    if config.cluster_mode {
        info!("Running in cluster mode");
        info!("Cluster config path: {:?}", config.cluster_config_path);
        
        // Load cluster configuration
        let cluster_config_path = config.cluster_config_path.to_str().unwrap_or("cluster.toml");
        let mut cluster_config = match load_cluster_config(cluster_config_path) {
            Ok(cfg) => {
                info!("Loaded cluster configuration from {}", cluster_config_path);
                cfg
            }
            Err(e) => {
                warn!("Failed to load cluster configuration from {}: {}", cluster_config_path, e);
                warn!("Using default cluster configuration");
                ClusterConfig::default()
            }
        };
        
        // Override node settings with command-line arguments if provided
        if let Some(ref node_id) = config.node_id {
            info!("Overriding node ID from command-line: {}", node_id);
            cluster_config.node.id = node_id.clone();
        }
        
        // If command-line options were provided, they override the config
        // But we always honor the internal port (node.port) from the config file
        // This ensures each node can have its own unique cluster communication port
        // while allowing the API ports to be specified from the command line
        
        info!("Using host: {}, API port: {}, cluster port: {}", 
              config.host, config.port, cluster_config.node.port);
              
        // Override the host and API port from command line
        cluster_config.node.host = config.host.clone();
        cluster_config.node.api_port = config.port;
        
        // Initialize the cluster
        let mut cluster = FastbuCluster::new(cluster_config.clone());
        match cluster.initialize().await {
            Ok(_) => info!("Cluster initialized successfully"),
            Err(e) => {
                warn!("Failed to initialize cluster: {}", e);
                warn!("Falling back to standalone mode");
                // Fall back to standalone mode
                run_standalone_mode(config).await?;
                return Ok(());
            }
        }
        
        // Create a cluster-aware cache
        let cluster_cache = ClusterCache::new(cluster);
        info!("Cluster-aware cache initialized successfully");
        
        // Start the server with cluster-aware cache
        info!("Starting server in cluster mode on {}:{}", config.host, config.port);
        
        // Start the HTTP API server with cluster-aware cache
        info!("Starting HTTP API server with cluster-aware cache");
        
        // Create a new API handler that wraps the cluster cache
        // Here we'll need to adapt the API module to work with the cluster cache
        let cluster_cache_arc = Arc::new(cluster_cache);
        let api_cache = ClusterAwareApiCache::new(Arc::clone(&cluster_cache_arc));
        
        crate::api::start_server(api_cache, config.host, config.port).await?;
    } else {
        // Run in standalone mode
        run_standalone_mode(config).await?;
    }

    info!("Server shutdown gracefully");
    Ok(())
}

async fn run_standalone_mode(config: Config) -> Result<(), Box<dyn Error>> {
    // Initialize the cache
    let cache = FastbuCache::new();
    info!("Cache initialized successfully");

    // Start the server
    info!("Starting server on {}:{}", config.host, config.port);

    // Use the ? operator to propagate errors
    crate::api::start_server(cache, config.host, config.port).await?;
    
    Ok(())
}
