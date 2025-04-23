use crate::cache::FastbuCache;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use warp::reject::Reject;
use warp::Filter;
use warp::Rejection;

// Custom error type that implements Reject
#[derive(Debug)]
struct CacheError(String);

impl Reject for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(err: std::io::Error) -> Self {
        CacheError(err.to_string())
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    if err.is_not_found() {
        Ok(warp::reply::with_status("NOT_FOUND", warp::http::StatusCode::NOT_FOUND))
    } else {
        Ok(warp::reply::with_status("INTERNAL_SERVER_ERROR", warp::http::StatusCode::INTERNAL_SERVER_ERROR))
    }
}


pub async fn start_server(cache: FastbuCache, host: String, port: u16) -> Result<(), warp::Error> {
    info!("Initializing server with host: {} and port: {}", host, port);
    let cache = Arc::new(cache);

    /*
     * GET /get/{key} - Retrieves the value associated with a given key from the cache.
     * If the key is not found, returns 404 Not Found.
     */
    let get_cache = cache.clone();
    let get_item = warp::path!("get" / String)
        .and(warp::any().map(move || get_cache.clone()))
        .and_then(|key: String, cache: Arc<FastbuCache>| {
            debug!("Received GET request for key: {}", key);
            let value = cache.get(&key);
            async move {
                if let Some(val) = value {
                    info!("Key found: {}. Returning value.", key);
                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&val),
                        warp::http::StatusCode::OK,
                    ))
                } else {
                    warn!("Key not found: {}", key);
                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&"Key not found"),
                        warp::http::StatusCode::NOT_FOUND,
                    ))
                }
            }
        });

    /*
     * POST /set/{key}/{value} - Stores a key-value pair in the cache.
     * Returns 200 OK upon successful insertion.
     */
    let set_cache = cache.clone();
    let set_item = warp::path!("set" / String / String)
        .and(warp::post())
        .and(warp::any().map(move || set_cache.clone()))
        .and_then(|key: String, value: String, cache: Arc<FastbuCache>| {
            debug!("Received POST request to set key: {} with value: {}", key, value);
            async move {
                debug!("Calling cache.insert for key: {}", key);
                match cache.insert(key.clone(), value.clone()) {
                    Ok(_) => {
                        debug!("Successfully inserted key: {}", key);
                        // Explicitly return a response
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&format!("Key '{}' stored successfully", key)),
                            warp::http::StatusCode::OK,
                        ))
                    }
                    Err(e) => {
                        error!("Failed to insert key: {}. Error: {}", key, e);
                        // Explicitly return an error response
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&format!("Failed to store key '{}'", key)),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        ))
                    }
                }
            }
        });

    let routes = get_item.or(set_item)
        .recover(handle_rejection)
        .with(warp::log("fastbu_cache"));

    info!("Starting Warp server on {}:{}", host, port);
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid address");
    warp::serve(routes).run(addr).await;
    info!("Server stopped.");
    Ok(())
}
