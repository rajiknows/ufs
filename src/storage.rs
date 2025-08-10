use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CHUNKS_CF: &str = "chunks";
const METADATA_CF: &str = "metadata";

/// Represents the metadata for a single file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub chunk_hashes: Vec<Vec<u8>>,
}

/// Manages the RocksDB database.
pub struct Storage {
    db: DB,
}

impl Storage {
    /// Opens the database at the given path.
    /// Creates column families if they don't exist.
    pub fn new(path: &str) -> Result<Self, rocksdb::Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db = DB::open_cf(&opts, path, vec![CHUNKS_CF, METADATA_CF])?;
        Ok(Storage { db })
    }

    /// Stores a raw data chunk, keyed by its SHA256 hash.
    pub fn store_chunk(&self, hash: &[u8], data: &[u8]) -> Result<(), rocksdb::Error> {
        let cf = self.db.cf_handle(CHUNKS_CF).unwrap();
        self.db.put_cf(cf, hash, data)
    }

    /// Retrieves a chunk by its hash.
    pub fn get_chunk(&self, hash: &[u8]) -> Result<Option<Vec<u8>>, rocksdb::Error> {
        let cf = self.db.cf_handle(CHUNKS_CF).unwrap();
        self.db.get_cf(cf, hash)
    }

    /// Stores serialized file metadata, keyed by its hash.
    pub fn store_metadata(
        &self,
        hash: &[u8],
        metadata: &FileInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cf = self.db.cf_handle(METADATA_CF).unwrap();
        let serialized = bincode::serialize(metadata)?;
        self.db.put_cf(cf, hash, serialized)?;
        Ok(())
    }

    /// Retrieves and deserializes file metadata by its hash.
    pub fn get_metadata(
        &self,
        hash: &[u8],
    ) -> Result<Option<FileInfo>, Box<dyn std::error::Error>> {
        let cf = self.db.cf_handle(METADATA_CF).unwrap();
        match self.db.get_cf(cf, hash)? {
            Some(data) => {
                let metadata: FileInfo = bincode::deserialize(&data)?;
                Ok(Some(metadata))
            }
            None => Ok(None),
        }
    }
}

/// Computes the SHA256 hash of a byte slice.
pub fn hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
