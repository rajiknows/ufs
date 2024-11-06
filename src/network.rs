use crate::fs::FileSystem;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

const CHUNK_SIZE: usize = 256 * 1024; // 256KB chunks
const MAX_REPLICAS: usize = 4;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

type NodeId = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Peer {
    pub id: NodeId,
    pub addr: SocketAddr,
    pub last_seen: u64, // Unix timestamp
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileChunk {
    hash: [u8; 32],
    data: Vec<u8>,
    chunk_index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    name: String,
    total_size: usize,
    chunk_hashes: Vec<[u8; 32]>,
    total_chunks: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
enum Message {
    UploadFile {
        name: String,
        data: Vec<u8>,
    },
    UploadChunk {
        metadata: FileMetadata,
        chunk: FileChunk,
    },
    GetFile {
        hash: [u8; 32],
    },
    GetChunk {
        file_hash: [u8; 32],
        chunk_index: usize,
    },
    ChunkRequest {
        chunk_hash: [u8; 32],
    },
    ChunkResponse {
        chunk: FileChunk,
    },
    ChunkBroadcast {
        chunk_hash: [u8; 32],
        holder: SocketAddr,
    },
    FileData {
        name: String,
        data: Vec<u8>,
    },
    ChunkData {
        chunk: FileChunk,
    },
    NotFound,
    DiscoverNodes,
    NodeList {
        nodes: Vec<Peer>,
    },
    Ping,
    Pong {
        id: NodeId,
    },
    Echo {
        msg: String,
    },
}

#[derive(Clone)]
pub struct Network {
    client_peer: Peer,
    nodes: Arc<Mutex<Vec<Peer>>>,
    fs: Arc<Mutex<FileSystem>>,
    chunk_store: Arc<Mutex<Vec<FileChunk>>>,
    chunk_locations: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
}

impl Network {
    pub async fn new(
        client_addr: SocketAddr,
        bootstrap_nodes: Vec<SocketAddr>,
    ) -> Result<Self, Box<dyn Error>> {
        let nodes = bootstrap_nodes
            .into_iter()
            .map(|addr| Peer {
                id: hash_socket_addr(&addr),
                addr,
                last_seen: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
            .collect();

        Ok(Network {
            client_peer: Peer {
                id: hash_socket_addr(&client_addr),
                addr: client_addr,
                last_seen: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            nodes: Arc::new(Mutex::new(nodes)),
            fs: Arc::new(Mutex::new(FileSystem::new())),
            chunk_store: Arc::new(Mutex::new(Vec::new())),
            chunk_locations: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn handle_connection(
        mut socket: TcpStream,
        fs: Arc<Mutex<FileSystem>>,
        nodes: Arc<Mutex<Vec<Peer>>>,
        chunk_store: Arc<Mutex<Vec<FileChunk>>>,
        chunk_locations: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
        client_addr: SocketAddr,
    ) {
        let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
        while let Ok(n) = socket.read(&mut buffer).await {
            if n == 0 {
                return;
            }
            if let Ok(message) = bincode::deserialize::<Message>(&buffer[..n]) {
                match message {
                    Message::UploadChunk { metadata, chunk } => {
                        // Store the chunk
                        let mut chunk_store = chunk_store.lock().await;
                        chunk_store.push(chunk.clone());

                        // Update chunk locations
                        let mut locations = chunk_locations.lock().await;
                        locations
                            .entry(chunk.hash)
                            .or_insert_with(Vec::new)
                            .push(client_addr);

                        // Broadcast chunk availability to network
                        Self::broadcast_chunk_location(&nodes, chunk.hash, client_addr).await;

                        let response =
                            bincode::serialize(&Message::ChunkResponse { chunk }).unwrap();
                        socket.write_all(&response).await.unwrap();
                    }
                    Message::GetChunk {
                        file_hash,
                        chunk_index,
                    } => {
                        let chunk_store = chunk_store.lock().await;
                        let response = if let Some(chunk) = chunk_store
                            .iter()
                            .find(|c| c.hash == file_hash && c.chunk_index == chunk_index)
                        {
                            bincode::serialize(&Message::ChunkData {
                                chunk: chunk.clone(),
                            })
                            .unwrap()
                        } else {
                            // If we don't have the chunk, check known locations
                            let locations = chunk_locations.lock().await;
                            if let Some(holders) = locations.get(&file_hash) {
                                // Forward request to known holders
                                for &holder_addr in holders {
                                    if holder_addr != client_addr {
                                        if let Ok(mut holder_stream) =
                                            TcpStream::connect(holder_addr).await
                                        {
                                            let request = bincode::serialize(&message).unwrap();
                                            holder_stream.write_all(&request).await.unwrap();
                                        }
                                    }
                                }
                            }
                            bincode::serialize(&Message::NotFound).unwrap()
                        };
                        socket.write_all(&response).await.unwrap();
                    }
                    Message::ChunkRequest { chunk_hash } => {
                        let chunk_store = chunk_store.lock().await;
                        if let Some(chunk) = chunk_store.iter().find(|c| c.hash == chunk_hash) {
                            let response = bincode::serialize(&Message::ChunkData {
                                chunk: chunk.clone(),
                            })
                            .unwrap();
                            socket.write_all(&response).await.unwrap();
                        } else {
                            // Forward request to known holders
                            let locations = chunk_locations.lock().await;
                            if let Some(holders) = locations.get(&chunk_hash) {
                                for &holder_addr in holders {
                                    if holder_addr != client_addr {
                                        if let Ok(mut holder_stream) =
                                            TcpStream::connect(holder_addr).await
                                        {
                                            let request =
                                                bincode::serialize(&Message::ChunkRequest {
                                                    chunk_hash,
                                                })
                                                .unwrap();
                                            holder_stream.write_all(&request).await.unwrap();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Message::ChunkBroadcast { chunk_hash, holder } => {
                        // Update chunk locations when we receive a broadcast
                        let mut locations = chunk_locations.lock().await;
                        locations
                            .entry(chunk_hash)
                            .or_insert_with(Vec::new)
                            .push(holder);
                    } // ... (rest of the message handling remains the same) ...
                }
            }
        }
    }

    async fn broadcast_chunk_location(
        nodes: &Arc<Mutex<Vec<Peer>>>,
        chunk_hash: [u8; 32],
        holder: SocketAddr,
    ) {
        let nodes = nodes.lock().await;
        let message = Message::ChunkBroadcast { chunk_hash, holder };
        let serialized = bincode::serialize(&message).unwrap();

        for peer in nodes.iter() {
            if peer.addr != holder {
                if let Ok(mut stream) = TcpStream::connect(peer.addr).await {
                    let _ = stream.write_all(&serialized).await;
                }
            }
        }
    }

    pub async fn get_chunk(
        &self,
        chunk_hash: &[u8; 32],
        chunk_index: usize,
    ) -> Result<Option<FileChunk>, Box<dyn Error>> {
        // First check local chunk store
        let chunk_store = self.chunk_store.lock().await;
        if let Some(chunk) = chunk_store.iter().find(|c| c.hash == *chunk_hash) {
            return Ok(Some(chunk.clone()));
        }
        drop(chunk_store);

        // If not found locally, check known locations
        let locations = self.chunk_locations.lock().await;
        if let Some(holders) = locations.get(chunk_hash) {
            for &holder_addr in holders {
                if let Ok(mut stream) = TcpStream::connect(holder_addr).await {
                    let request = Message::GetChunk {
                        file_hash: *chunk_hash,
                        chunk_index,
                    };
                    let serialized = bincode::serialize(&request)?;
                    stream.write_all(&serialized).await?;

                    let mut buffer = vec![0; 1024 * 1024];
                    let n = stream.read(&mut buffer).await?;

                    if let Ok(Message::ChunkData { chunk }) = bincode::deserialize(&buffer[..n]) {
                        return Ok(Some(chunk));
                    }
                }
            }
        }

        // If still not found, broadcast request to network
        self.broadcast_chunk_request(chunk_hash).await?;

        Ok(None)
    }

    async fn broadcast_chunk_request(&self, chunk_hash: &[u8; 32]) -> Result<(), Box<dyn Error>> {
        let nodes = self.nodes.lock().await;
        let request = Message::ChunkRequest {
            chunk_hash: *chunk_hash,
        };
        let serialized = bincode::serialize(&request)?;

        for peer in nodes.iter() {
            if peer.addr != self.client_peer.addr {
                if let Ok(mut stream) = TcpStream::connect(peer.addr).await {
                    let _ = stream.write_all(&serialized).await;
                }
            }
        }
        Ok(())
    }

    // ... (rest of the implementation remains the same) ...
}

fn hash_socket_addr(addr: &SocketAddr) -> NodeId {
    let mut hasher = Sha256::new();
    hasher.update(addr.to_string().as_bytes());
    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = a[i] ^ b[i];
    }
    result
}

// ... (helper functions remain the same) ...
