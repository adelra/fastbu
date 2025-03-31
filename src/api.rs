use crate::cache::FastbuCache;
use std::sync::Arc;
use warp::Filter;

pub async fn start_server(cache: FastbuCache) -> Result<(), warp::Error> {
    /// Starts the web server and listens for incoming requests on localhost:3030.
    let cache = Arc::new(cache);

    /**
     * GET /get/{key} - Retrieves the value associated with a given key from the cache.
     * If the key is not found, returns 404 Not Found.
     */
     let get_item = warp::path!("get" / String)
     .and(warp::any().map(move || cache.clone()))
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
    let set_item = warp::path!("set" / String / String)
        .and(warp::any().map(move || cache.clone()))
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
     * Starts the Warp server on localhost:3030 and listens for incoming requests.
     */
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}
