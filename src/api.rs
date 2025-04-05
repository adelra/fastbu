use crate::cache::FastbuCache;
use std::sync::Arc;
use warp::Filter;

pub async fn start_server(cache: FastbuCache, host: String, port: u16) -> Result<(), warp::Error> {
    /// Starts the web server and listens for incoming requests.
    let cache = Arc::new(cache);

    /**
     * GET /get/{key} - Retrieves the value associated with a given key from the cache.
     * If the key is not found, returns 404 Not Found.
     */
    let get_cache = cache.clone(); // Clone the Arc for the GET route
    let get_item = warp::path!("get" / String)
        .and(warp::any().map(move || get_cache.clone())) // Use the cloned Arc
        .and_then(|key: String, cache: Arc<FastbuCache>| {
            let value = cache.get(&key);
            async move {
                if let Some(val) = value {
                    Ok::<_, warp::Rejection>(warp::reply::json(&val))
                } else {
                    Err(warp::reject::not_found())
                }
            }
        });

    /**
     * POST /set/{key}/{value} - Stores a key-value pair in the cache.
     * Returns 200 OK upon successful insertion.
     */
    let set_cache = cache.clone(); // Clone the Arc for the POST route
    let set_item = warp::path!("set" / String / String)
        .and(warp::any().map(move || set_cache.clone())) // Use the cloned Arc
        .and_then(|key: String, value: String, cache: Arc<FastbuCache>| {
            cache.insert(key, value);
            async move {
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&"OK"),
                    warp::http::StatusCode::OK,
                ))
            }
        });

    let routes = get_item.or(set_item);

    /**
     * Starts the Warp server and listens for incoming requests.
     */
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    Ok(())
}