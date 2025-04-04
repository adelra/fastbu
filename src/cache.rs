use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Debug)]
pub struct CacheEntry {
    value: String,
}

pub struct FastbuCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl FastbuCache {
    pub fn new() -> Self {
        FastbuCache {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, key: String, value: String) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(key, CacheEntry { value });
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.lock().unwrap();
        cache.get(key).map(|entry| entry.value.clone())
    }

    pub fn remove(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(key);
    }
}
