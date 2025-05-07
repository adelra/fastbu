mod api;
mod cache;
mod storage;

use crate::cache::FastbuCache;
use clap::Parser;
use env_logger::Builder;
use log::{info, LevelFilter};
use std::error::Error;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3031;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 3031)]
    port: u16,
}

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
        .filter_level(LevelFilter::Debug) // Change from Info to Debug
        .parse_filters(&std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string())) // Respect RUST_LOG
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

    info!("Server configuration:");
    info!("Host: {}", config.host);
    info!("Port: {}", config.port);

    // Initialize the cache
    let cache = FastbuCache::new();
    info!("Cache initialized successfully");

    // Start the server
    info!("Starting server on {}:{}", config.host, config.port);

    // Use the ? operator to propagate errors
    crate::api::start_server(cache, config.host, config.port).await?;

    info!("Server shutdown gracefully");
    Ok(())
}
