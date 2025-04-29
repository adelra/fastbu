use crate::cache::CacheEntry;
use chrono::{DateTime, Utc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex; // Add logging

const STORAGE_DIR: &str = "cache_storage";
const INDEX_FILE: &str = "cache_index.bin";

#[derive(Debug, Serialize, Deserialize)]
struct StorageMetadata {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexEntry {
    key: String,
    file_path: String,
    metadata: StorageMetadata,
}

pub struct Storage {
    base_dir: PathBuf,
    index_file: PathBuf,
    index: Mutex<Vec<IndexEntry>>,
}

impl Storage {
    pub fn new() -> io::Result<Self> {
        let base_dir = PathBuf::from(STORAGE_DIR);
        let index_file = base_dir.join(INDEX_FILE);

        // Create storage directory if it doesn't exist
        std::fs::create_dir_all(&base_dir)?;

        let storage = Storage {
            base_dir,
            index_file,
            index: Mutex::new(Vec::new()),
        };

        // Load existing index if it exists
        if storage.index_file.exists() {
            storage.load_index()?;
        }

        Ok(storage)
    }

    fn load_index(&self) -> io::Result<()> {
        let mut file = File::open(&self.index_file)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        if !contents.is_empty() {
            let index: Vec<IndexEntry> = bincode::deserialize(&contents)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            *self.index.lock().unwrap() = index;
        }

        Ok(())
    }

    fn save_index(&self) -> io::Result<()> {
        let index = self.index.lock().unwrap();
        let data = bincode::serialize(&*index)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut file = File::create(&self.index_file)?;
        file.write_all(&data)?;
        Ok(())
    }

    pub fn save(&self, key: &str, entry: &CacheEntry) -> io::Result<()> {
        debug!("Starting save operation for key: {}", key);

        let file_path = self.base_dir.join(format!("{}.cache", key));
        debug!("Resolved file path for key: {}: {:?}", key, file_path);

        let metadata = StorageMetadata {
            created_at: Utc::now(),
            updated_at: Utc::now(),
            size: 0, // Will be updated after writing
        };

        // Serialize the entry
        let data = match bincode::serialize(entry) {
            Ok(data) => {
                debug!("Successfully serialized entry for key: {}", key);
                data
            }
            Err(e) => {
                error!("Failed to serialize entry for key: {}. Error: {}", key, e);
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        };

        // Write to file
        debug!(
            "Attempting to write serialized data to file for key: {}",
            key
        );
        let mut file = match OpenOptions::new()
            .append(true)
            .create(true)
            .open(&file_path)
        {
            Ok(file) => file,
            Err(e) => {
                error!("Failed to open file for key: {}. Error: {}", key, e);
                return Err(e);
            }
        };

        if let Err(e) = file.write_all(&data) {
            error!(
                "Failed to write data to file for key: {}. Error: {}",
                key, e
            );
            return Err(e);
        }
        debug!("Successfully wrote data to file for key: {}", key);

        let size = match file.metadata() {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                error!(
                    "Failed to retrieve file metadata for key: {}. Error: {}",
                    key, e
                );
                return Err(e);
            }
        };

        // Update index
        debug!("Updating index for key: {}", key);
        let mut index = self.index.lock().unwrap();
        if let Some(existing) = index.iter_mut().find(|e| e.key == key) {
            existing.metadata.updated_at = Utc::now();
            existing.metadata.size = size;
        } else {
            index.push(IndexEntry {
                key: key.to_string(),
                file_path: file_path.to_string_lossy().into_owned(),
                metadata,
            });
        }

        // Save the updated index
        debug!("Saving updated index for key: {}", key);
        debug!("Index size after update: {}", index.len());
        info!("Successfully completed save operation for key: {}", key);
        Ok(())
    }

    pub fn load(&self, key: &str) -> io::Result<Option<CacheEntry>> {
        let index = self.index.lock().unwrap();
        if let Some(entry) = index.iter().find(|e| e.key == key) {
            let mut file = File::open(&entry.file_path)?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;

            let cache_entry: CacheEntry = bincode::deserialize(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            Ok(Some(cache_entry))
        } else {
            Ok(None)
        }
    }

    pub fn remove(&self, key: &str) -> io::Result<()> {
        let mut index = self.index.lock().unwrap();
        if let Some(pos) = index.iter().position(|e| e.key == key) {
            let entry = &index[pos];
            if Path::new(&entry.file_path).exists() {
                std::fs::remove_file(&entry.file_path)?;
            }
            index.remove(pos);
            self.save_index()?;
        }
        Ok(())
    }

    pub fn cleanup(&self) -> io::Result<()> {
        let index = self.index.lock().unwrap();
        for entry in index.iter() {
            if !Path::new(&entry.file_path).exists() {
                // File doesn't exist, could be cleaned up from index
                // This would require modifying the index, so we'll just log it
                eprintln!("Warning: Cache file {} not found", entry.file_path);
            }
        }
        Ok(())
    }
}
