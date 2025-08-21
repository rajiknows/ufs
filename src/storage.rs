use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Represents the metadata for a single file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub chunk_hashes: Vec<Vec<u8>>,
}

/// Manages the in-memory storage.
#[derive(Clone, Default)]
pub struct Storage {
    chunks: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    metadata: Arc<RwLock<HashMap<Vec<u8>, FileInfo>>>,
    dht_values: Arc<RwLock<HashMap<Vec<u8>, String>>>,
}

impl Storage {
    /// Creates a new in-memory storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a raw data chunk, keyed by its SHA256 hash.
    pub fn store_chunk(&self, hash: &[u8], data: &[u8]) {
        self.chunks
            .write()
            .unwrap()
            .insert(hash.to_vec(), data.to_vec());
    }

    /// Retrieves a chunk by its hash.
    pub fn get_chunk(&self, hash: &[u8]) -> Option<Vec<u8>> {
        self.chunks.read().unwrap().get(hash).cloned()
    }

    /// Stores file metadata, keyed by its hash.
    pub fn store_metadata(&self, hash: &[u8], metadata: &FileInfo) {
        self.metadata
            .write()
            .unwrap()
            .insert(hash.to_vec(), metadata.clone());
    }

    /// Retrieves file metadata by its hash.
    pub fn get_metadata(&self, hash: &[u8]) -> Option<FileInfo> {
        self.metadata.read().unwrap().get(hash).cloned()
    }

    /// Retrieves all file metadata from the database.
    pub fn get_all_metadata(&self) -> Vec<FileInfo> {
        self.metadata.read().unwrap().values().cloned().collect()
    }

    // New methods for DHT values
    pub fn store_value(&self, key: &[u8], value: &str) {
        self.dht_values
            .write()
            .unwrap()
            .insert(key.to_vec(), value.to_string());
    }

    pub fn get_value(&self, key: &[u8]) -> Option<String> {
        self.dht_values.read().unwrap().get(key).cloned()
    }
}
