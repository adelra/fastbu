use warp::Filter;
use crate::cache::FastbuCache;
use std::sync::Arc;

pub async fn start_server(cache: FastbuCache) -> Result<(), warp::Error> {
    let cache = Arc::new(cache);

    let get_item = warp::path!("get" / String)
        .and(warp::any().map(move || cache.clone()))
        .and_then(|key: String, cache: Arc<FastbuCache>| {
            let value = cache.get(&key);
            async move {
                match value {
                    Some(val) => Ok::<_, warp::Rejection>(warp::reply::json(&val)),
                    None => Ok(warp::reply::with_status("Not Found".to_string(), warp::http::StatusCode::NOT_FOUND)),
                }
            }
        });

    let set_item = warp::path!("set" / String / String)
        .and(warp::any().map(move || cache.clone()))
        .and_then(|key: String, value: String, cache: Arc<FastbuCache>| {
            cache.insert(key, value);
            async move {
                Ok::<_, warp::Rejection>(warp::reply::with_status("OK".to_string(), warp::http::StatusCode::OK))
            }
        });

    let routes = get_item.or(set_item);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

