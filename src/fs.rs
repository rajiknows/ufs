use sha2::{Digest, Sha256};
const CHUNK_SIZE: usize = 256 * 1024; // 256 KB chunks
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type ChunkHash = [u8; 32];
type Chunk = Vec<u8>;

/// CAS is content addresible storage
///
#[derive(Debug, Clone)]
pub struct CAS {
    storage: HashMap<ChunkHash, Chunk>,
}

impl CAS {
    pub fn new() -> Self {
        CAS {
            storage: HashMap::new(),
        }
    }

    pub fn add(&mut self, data: Vec<u8>) -> [u8; 32] {
        let hash = self.hash(&data);
        self.storage.insert(hash, data);
        hash
    }

    pub fn get(&self, hash: &[u8; 32]) -> Option<&Vec<u8>> {
        self.storage.get(hash)
    }

    fn hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub date: usize,
    pub size: usize,
    pub author: String,
    pub total_size: usize,           // Added for network operations
    pub chunk_hashes: Vec<[u8; 32]>, // Moved from FileObject to FileInfo
}

impl FileInfo {
    pub fn new(
        name: String,
        date: usize,
        size: usize,
        author: String,
        total_size: usize,
        chunk_hashes: Vec<[u8; 32]>,
    ) -> FileInfo {
        Self {
            name,
            date,
            size,
            author,
            total_size,
            chunk_hashes,
        }
    }
}
#[derive(Debug, Clone)]
pub struct FileSystem {
    files: HashMap<[u8; 32], FileInfo>, // Changed from Vec<String> to track files by hash
    cas: Arc<Mutex<CAS>>,
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            files: HashMap::new(),
            cas: Arc::new(Mutex::new(CAS::new())),
        }
    }

    pub async fn add_file(&mut self, name: &str, data: &[u8]) -> [u8; 32] {
        let chunks: Vec<_> = data.chunks(CHUNK_SIZE).collect();
        let mut cas = self.cas.lock().await;

        let chunk_hashes: Vec<_> = chunks.iter().map(|chunk| cas.add(chunk.to_vec())).collect();

        let file_info = FileInfo::new(
            name.to_string(),
            1234, // TODO: Use actual timestamp
            data.len(),
            "raj".to_string(),
            data.len(),
            chunk_hashes,
        );

        let file_info_bytes = bincode::serialize(&file_info).unwrap();
        let file_hash = cas.add(file_info_bytes);

        self.files.insert(file_hash, file_info);
        file_hash
    }

    pub fn get_file_metadata(&self, file_hash: &[u8; 32]) -> Option<&FileInfo> {
        self.files.get(file_hash)
    }

    pub async fn get_chunk(&self, chunk_hash: &[u8; 32]) -> Option<Vec<u8>> {
        let cas = self.cas.lock().await;
        cas.get(chunk_hash).map(|v| v.clone())
    }

    pub async fn add_chunk(&mut self, _chunk_hash: [u8; 32], data: Vec<u8>) {
        let mut cas = self.cas.lock().await;
        cas.add(data);
    }

    pub fn list_files(&self) -> Vec<([u8; 32], String)> {
        self.files
            .iter()
            .map(|(hash, info)| (*hash, info.name.clone()))
            .collect()
    }
}
