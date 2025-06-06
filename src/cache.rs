use crate::storage::Storage;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex}; // Add logging and Arc
use tokio::task;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheEntry {
    value: String,
}

pub struct FastbuCache {
    data: Arc<Mutex<CacheData>>,
}

impl Clone for FastbuCache {
    fn clone(&self) -> Self {
        FastbuCache {
            data: Arc::clone(&self.data),
        }
    }
}

struct CacheData {
    cache: HashMap<String, CacheEntry>,
    storage: Storage,
}

impl FastbuCache {
    pub fn new() -> Self {
        FastbuCache {
            data: Arc::new(Mutex::new(CacheData {
                cache: HashMap::new(),
                storage: Storage::new().unwrap(),
            })),
        }
    }

    pub async fn insert(&self, key: String, value: String) -> Result<(), std::io::Error> {
        debug!("Attempting to insert key: {} with value: {}", key, value);

        let entry = CacheEntry {
            value: value.clone(),
        };
        
        // Create clones for the spawn_blocking operation
        let key_clone = key.clone();
        let entry_clone = entry.clone();
        
        {
            // Update in-memory cache - acquire lock in this smaller scope
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
            
            data.cache.insert(key.clone(), entry.clone());
            debug!("In-memory cache updated for key: {}", key);
        }
        
        // Clone the self reference to move into spawn_blocking
        let self_clone = self.clone();
        
        // Persist to disk using spawn_blocking to avoid blocking the async runtime
        debug!("Attempting to persist key: {} to disk", key);
        let result = task::spawn_blocking(move || {
            let data = match self_clone.data.lock() {
                Ok(lock) => lock,
                Err(_e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Lock poisoned inside spawn_blocking",
                    ));
                }
            };
            data.storage.save(&key_clone, &entry_clone)
        }).await.unwrap_or_else(|e| {
            error!("Task join error when persisting key: {}. Error: {}", key, e);
            Err(std::io::Error::new(std::io::ErrorKind::Other, e))
        });
        
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

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = FastbuCache::new();

        let key = "test_key".to_string();
        let value = "test_value".to_string();

        // Insert the key-value pair
        assert!(cache.insert(key.clone(), value.clone()).await.is_ok());

        // Retrieve the value
        let retrieved_value = cache.get(&key);
        assert!(retrieved_value.is_some());
        assert_eq!(retrieved_value.unwrap(), value);
    }

    #[tokio::test]
    async fn test_get_nonexistent_key() {
        let cache = FastbuCache::new();

        let key = "nonexistent_key";

        // Attempt to retrieve a nonexistent key
        let retrieved_value = cache.get(key);
        assert!(retrieved_value.is_none());
    }
}
