mod api;
mod cache;
mod storage;
mod utils;

use crate::cache::FastbuCache;
use crate::api::start_server;
use warp::Filter;
use std::net::SocketAddr;
use log::{info, error, LevelFilter};
use env_logger::Builder;
use std::error::Error;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3031;

#[derive(Debug)]
struct Config {
    host: String,
    port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
        }
    }
}

fn setup_logging() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .init();
}

fn parse_args() -> Config {
    let args: Vec<String> = std::env::args().collect();
    let mut config = Config::default();

    for i in 1..args.len() {
        match args[i].as_str() {
            "--host" | "-h" => {
                if i + 1 < args.len() {
                    config.host = args[i + 1].clone();
                }
            }
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    if let Ok(port) = args[i + 1].parse() {
                        config.port = port;
                    }
                }
            }
            _ => {}
        }
    }

    config
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Setup logging
    setup_logging();
    info!("Starting Fastbu cache server...");

    // Parse configuration
    let config = parse_args();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    
    info!("Server configuration:");
    info!("Host: {}", config.host);
    info!("Port: {}", config.port);

    // Initialize the cache
    let cache = FastbuCache::new();
    info!("Cache initialized successfully");

    // Start the server
    info!("Starting server on {}:{}", config.host, config.port);
    
    // Use the ? operator to propagate errors
    start_server(cache, config.host, config.port).await?;
    
    info!("Server shutdown gracefully");
    Ok(())
}
