use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use serde::{Serialize, Deserialize};
use crate::cache::CacheEntry;

pub fn save_to_disk(key: &str, entry: &CacheEntry) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("{}.cache", key))?;
    let data = serde_json::to_string(entry)?;
    file.write_all(data.as_bytes())?;
    Ok(())
}

pub fn load_from_disk(key: &str) -> std::io::Result<Option<CacheEntry>> {
    let mut file = File::open(format!("{}.cache", key))?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    let entry: CacheEntry = serde_json::from_str(&data)?;
    Ok(Some(entry))
}

