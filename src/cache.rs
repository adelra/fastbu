use crate::storage::Storage;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex; // Add logging

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheEntry {
    value: String,
}

pub struct FastbuCache {
    data: Mutex<CacheData>,
}

struct CacheData {
    cache: HashMap<String, CacheEntry>,
    storage: Storage,
}

impl FastbuCache {
    pub fn new() -> Self {
        FastbuCache {
            data: Mutex::new(CacheData {
                cache: HashMap::new(),
                storage: Storage::new().unwrap(),
            }),
        }
    }

    pub fn insert(&self, key: String, value: String) -> Result<(), std::io::Error> {
        debug!("Attempting to insert key: {} with value: {}", key, value);

        let entry = CacheEntry {
            value: value.clone(),
        };

        let mut data = match self.data.lock() {
            Ok(lock) => lock,
            Err(e) => {
                error!("Failed to acquire lock on data: {}", e);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Lock poisoned",
                ));
            }
        };

        // Update in-memory cache
        data.cache.insert(key.clone(), entry.clone());
        debug!("In-memory cache updated for key: {}", key);

        // Persist to disk
        debug!("Attempting to persist key: {} to disk", key);
        let result = data.storage.save(&key, &entry);
        if result.is_ok() {
            info!("Successfully persisted key:   {} to disk", key);
        } else {
            error!("Failed to persist key: {} to disk", key);
        }
        result
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.data.lock().unwrap();
        data.cache.get(key).map(|entry| entry.value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let cache = FastbuCache::new();

        let key = "test_key".to_string();
        let value = "test_value".to_string();

        // Insert the key-value pair
        assert!(cache.insert(key.clone(), value.clone()).is_ok());

        // Retrieve the value
        let retrieved_value = cache.get(&key);
        assert!(retrieved_value.is_some());
        assert_eq!(retrieved_value.unwrap(), value);
    }

    #[test]
    fn test_get_nonexistent_key() {
        let cache = FastbuCache::new();

        let key = "nonexistent_key";

        // Attempt to retrieve a nonexistent key
        let retrieved_value = cache.get(key);
        assert!(retrieved_value.is_none());
    }
}
