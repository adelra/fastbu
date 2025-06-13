use crate::api_cache_trait::ApiCache;
use crate::cache::FastbuCache;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use warp::Filter;

async fn handle_rejection(
    err: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    if err.is_not_found() {
        Ok(warp::reply::with_status(
            "NOT_FOUND",
            warp::http::StatusCode::NOT_FOUND,
        ))
    } else {
        Ok(warp::reply::with_status(
            "INTERNAL_SERVER_ERROR",
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

pub async fn start_server<T: ApiCache + 'static>(cache: T, host: String, port: u16) -> Result<(), warp::Error> {
    info!("Initializing server with host: {} and port: {}", host, port);
    let cache = Arc::new(cache);

    /*
     * GET /get/{key} - Retrieves the value associated with a given key from the cache.
     * If the key is not found, returns 404 Not Found.
     */
    let get_cache = cache.clone();
    let get_item = warp::path!("get" / String)
        .and(warp::any().map(move || get_cache.clone()))
        .and_then(|key: String, cache: Arc<T>| {
            debug!("Received GET request for key: {}", key);
            async move {
                let value = cache.get(&key).await;
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
        .and_then(|key: String, value: String, cache: Arc<T>| {
            debug!(
                "Received POST request to set key: {} with value: {}",
                key, value
            );
            async move {
                debug!("Calling cache.set for key: {}", key);
                match cache.set(key.clone(), value.clone()).await {
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

    let routes = get_item
        .or(set_item)
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

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;
    use warp::http::StatusCode;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // A simple mock implementation of ApiCache for testing
    struct MockCache {
        data: Mutex<HashMap<String, String>>,
    }
    
    #[async_trait]
    impl ApiCache for MockCache {
        async fn get(&self, key: &str) -> Option<String> {
            if let Ok(data) = self.data.lock() {
                data.get(key).map(|s| s.clone())
            } else {
                None
            }
        }
        
        async fn set(&self, key: String, value: String) -> Result<(), std::io::Error> {
            if let Ok(mut data) = self.data.lock() {
                data.insert(key, value);
                Ok(())
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned"))
            }
        }
    }
    
    impl MockCache {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }
    
    #[tokio::test]
    async fn test_get_endpoint() {
        let mock_cache = MockCache::new();
        {
            let mut data = mock_cache.data.lock().unwrap();
            data.insert("test_key".to_string(), "test_value".to_string());
        }
        
        // Create the filter for testing
        let cache = Arc::new(mock_cache);
        let get_clone = cache.clone();
        
        let get_item = warp::path!("get" / String)
            .and(warp::any().map(move || get_clone.clone()))
            .and_then(|key: String, cache: Arc<MockCache>| {
                async move {
                    let value = cache.get(&key).await;
                    if let Some(val) = value {
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&val),
                            warp::http::StatusCode::OK,
                        ))
                    } else {
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&"Key not found"),
                            warp::http::StatusCode::NOT_FOUND,
                        ))
                    }
                }
            });
            
        // Test get endpoint with existing key
        let resp = request()
            .method("GET")
            .path("/get/test_key")
            .reply(&get_item)
            .await;
            
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.body(), "\"test_value\"");
        
        // Test get endpoint with non-existing key
        let resp = request()
            .method("GET")
            .path("/get/nonexistent")
            .reply(&get_item)
            .await;
            
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
