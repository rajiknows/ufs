type ChunkId = [u8; 32];

struct DataObject {
    merkle_root: [u8; 32],
    chunks: Vec<ChunkId>,
}

struct Chunks {
    id: ChunkId,
    data: Vec<u8>,
}
