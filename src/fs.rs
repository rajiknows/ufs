use sha2::{Digest, Sha256};
const CHUNK_SIZE: usize = 256 * 1024; // 256 KB chunks
use std::collections::HashMap;
type NodeID = [u8; 32];

pub struct CAS {
    storage: HashMap<NodeID, Vec<u8>>,
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

#[derive(serde::Serialize, serde::Deserialize)]
struct FileInfo {
    name: String,
    chunk_hashes: Vec<[u8; 32]>,
}

pub struct FileSystem {
    files: Vec<String>,
    cas: CAS,
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            files: Vec::new(),
            cas: CAS::new(),
        }
    }

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> [u8; 32] {
        let chunks: Vec<_> = data.chunks(CHUNK_SIZE).collect();
        let chunk_hashes: Vec<_> = chunks
            .iter()
            .map(|chunk| self.cas.add(chunk.to_vec()))
            .collect();

        let file_info = FileInfo {
            name: name.to_string(),
            chunk_hashes,
        };
        self.files.push(name.to_string());

        let file_info_bytes = bincode::serialize(&file_info).unwrap();
        self.cas.add(file_info_bytes)
    }

    // TODO: Implement file retrieval
    pub fn get_file(&self, file_hash: &[u8; 32]) -> Option<(String, Vec<u8>)> {
        let file_info_bytes = self.cas.get(file_hash)?;
        let file_info: FileInfo = bincode::deserialize(file_info_bytes).ok()?;

        let mut file_data = Vec::new();
        for chunk_hash in &file_info.chunk_hashes {
            if let Some(chunk) = self.cas.get(chunk_hash) {
                file_data.extend_from_slice(chunk);
            } else {
                return None; // If any chunk is missing, we can't retrieve the file
            }
        }

        Some((file_info.name, file_data))
    }
    // TODO: Implement file listing
    pub fn list_file(&self) -> Vec<String> {
        return self.files.clone();
    }
}
