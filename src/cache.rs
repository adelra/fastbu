use crate::storage::Storage;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex; // Add logging
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheEntry {
    value: String,
    ttl_seconds: DateTime<Utc>, // Add expiration time

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

    pub fn insert(&self, key: String, value: String, ttl_seconds: u64) -> Result<(), std::io::Error> {
        debug!("Attempting to insert key: {} with value: {}", key, value);

        let entry = CacheEntry {
            value: value.clone(),
            ttl_seconds: Utc::now() + chrono::Duration::seconds(ttl_seconds as i64),
        };
        let expiry = Utc::now() + chrono::Duration::seconds(ttl_seconds as i64);
        debug!(
            "Calculated expiry time for key: {} is: {}",
            key,
            expiry
        );
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

    pub fn cleanup(&self) -> Result<(), std::io::Error> {
        let mut data = self.data.lock().map_err(|e| {
            error!("Failed to acquire lock on data: {}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned")
        })?;
        let now = Utc::now();
        let expired: Vec<_> = data.cache.iter()
            .filter(|(_, entry)| entry.ttl_seconds < now)
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            data.cache.remove(&key);
            // Also remove from storage if needed
            self.delete_from_storage(&key)?;
        }
        Ok(())
    }
    fn delete_from_storage(&self, key: &str) -> Result<(), std::io::Error> {
        let data = match self.data.lock() {
            Ok(lock) => lock,
            Err(e) => {
                error!("Failed to acquire lock on data: {}", e);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Lock poisoned",
                ));
            }
        };
        data.storage.delete(key)
    }
}