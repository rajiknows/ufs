use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sled::{Db, Tree};

const CHUNKS_TREE: &str = "chunks";
const METADATA_TREE: &str = "metadata";

/// Represents the metadata for a single file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub chunk_hashes: Vec<Vec<u8>>,
}

/// Manages the Sled database.
pub struct Storage {
    db: Db,
    chunks: Tree,
    metadata: Tree,
}

impl Storage {
    /// Opens the database at the given path.
    pub fn new(path: &str) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        let chunks = db.open_tree(CHUNKS_TREE)?;
        let metadata = db.open_tree(METADATA_TREE)?;
        Ok(Storage { db, chunks, metadata })
    }

    /// Stores a raw data chunk, keyed by its SHA256 hash.
    pub fn store_chunk(&self, hash: &[u8], data: &[u8]) -> Result<(), sled::Error> {
        self.chunks.insert(hash, data)?;
        Ok(())
    }

    /// Retrieves a chunk by its hash.
    pub fn get_chunk(&self, hash: &[u8]) -> Result<Option<Vec<u8>>, sled::Error> {
        self.chunks.get(hash).map(|opt| opt.map(|v| v.to_vec()))
    }

    /// Stores serialized file metadata, keyed by its hash.
    pub fn store_metadata(
        &self,
        hash: &[u8],
        metadata: &FileInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = bincode::serialize(metadata)?;
        self.metadata.insert(hash, serialized)?;
        Ok(())
    }

    /// Retrieves and deserializes file metadata by its hash.
    pub fn get_metadata(
        &self,
        hash: &[u8],
    ) -> Result<Option<FileInfo>, Box<dyn std::error::Error>> {
        match self.metadata.get(hash)? {
            Some(data) => {
                let metadata: FileInfo = bincode::deserialize(&data)?;
                Ok(Some(metadata))
            }
            None => Ok(None),
        }
    }

    /// Retrieves all chunk hashes from the database.
    pub fn get_all_chunk_hashes(&self) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
        let mut hashes = Vec::new();
        for key in self.chunks.iter().keys() {
            hashes.push(key?.to_vec());
        }
        Ok(hashes)
    }

    /// Retrieves all file metadata from the database.
    pub fn get_all_metadata(&self) -> Result<Vec<FileInfo>, Box<dyn std::error::Error>> {
        let mut metadata = Vec::new();
        for item in self.metadata.iter() {
            let (_, value) = item?;
            let file_info: FileInfo = bincode::deserialize(&value)?;
            metadata.push(file_info);
        }
        Ok(metadata)
    }
}

/// Computes the SHA256 hash of a byte slice.
pub fn hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}