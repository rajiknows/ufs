use sha2::{Digest, Sha256};
const CHUNK_SIZE: usize = 256 * 1024; // 256 KB chunks
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type ChunkHash = [u8; 32];
type Chunk = Vec<u8>;

/// CAS is content addresible storage
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

    pub fn remove(&mut self, chunk_hash: [u8; 32]) -> () {
        self.storage.remove(&chunk_hash);
        ()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub filehash: [u8; 32],
    pub name: String,
    pub date: usize,
    pub size: usize,
    pub author: String,
    pub total_size: usize,           // Added for network operations
    pub chunk_hashes: Vec<[u8; 32]>, // Moved from FileObject to FileInfo
}

impl FileInfo {
    pub fn new(
        filehash: [u8; 32],
        name: String,
        date: usize,
        size: usize,
        author: String,
        total_size: usize,
        chunk_hashes: Vec<[u8; 32]>,
    ) -> FileInfo {
        Self {
            filehash,
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
            hash_file(data),
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

    pub async fn delete_file(&mut self, file_hash: [u8; 32]) -> () {
        let mut cas = self.cas.lock().await;
        cas.remove(file_hash);
        drop(cas);
        self.files.remove(&file_hash);
        ()
    }

    pub fn get_file_metadata(&self, file_hash: &[u8; 32]) -> Option<&FileInfo> {
        self.files.get(file_hash)
    }

    pub fn add_file_metadata(&mut self, file_hash: [u8; 32], file_info: FileInfo) {
        self.files.insert(file_hash, file_info);
    }

    pub async fn get_chunk(&self, chunk_hash: &[u8; 32]) -> Option<Vec<u8>> {
        let cas = self.cas.lock().await;
        cas.get(chunk_hash).map(|v| v.clone())
    }

    //pub async fn add_chunk(&mut self, _chunk_hash: [u8; 32], data: Vec<u8>) {
    //    let mut cas = self.cas.lock().await;
    //    cas.add(data);
    //}

    pub fn list_files(&self) -> Vec<([u8; 32], String)> {
        self.files
            .iter()
            .map(|(hash, info)| (*hash, info.name.clone()))
            .collect()
    }

    pub async fn add_file_from_path(&mut self, path: &str) -> [u8; 32] {
        let data = tokio::fs::read(path).await.expect("Failed to read file");
        self.add_file(path, &data).await
    }
}
pub fn hash_file(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
