use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::fs::{FileInfo, FileSystem};

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    GetFile { file_hash: [u8; 32] },
    GetChunk { chunk_hash: [u8; 32] },
    FileMetadata { metadata: FileInfo },
    ChunkData { chunk: Vec<u8> },
    ListFiles,
    FileList { files: Vec<([u8; 32], String)> },
    Error { message: String },
    NewFile { file_hash: [u8; 32] },
    AddPeer { peer_addr: SocketAddr },
}
#[derive(Debug, Clone)]
pub struct NetworkNode {
    fs: Arc<Mutex<FileSystem>>,
    known_peers: Arc<Mutex<Vec<SocketAddr>>>,
    chunk_locations: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
    addr: SocketAddr,
}

impl NetworkNode {
    pub fn new(addr: SocketAddr) -> Self {
        NetworkNode {
            fs: Arc::new(Mutex::new(FileSystem::new())),
            known_peers: Arc::new(Mutex::new(Vec::new())),
            chunk_locations: Arc::new(Mutex::new(HashMap::new())),
            addr,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(self.addr)?;
        println!("Listening on {}", self.addr);

        loop {
            let (socket, peer_addr) = listener.accept()?;
            println!("New connection from {}", peer_addr);

            let fs = self.fs.clone();
            let chunk_locations = self.chunk_locations.clone();
            let known_peers = self.known_peers.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_connection(socket, fs, chunk_locations, known_peers).await
                {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        mut socket: TcpStream,
        fs: Arc<Mutex<FileSystem>>,
        _chunk_locations: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
        known_peers: Arc<Mutex<Vec<SocketAddr>>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
        let n = socket.read(&mut buffer)?;

        let message: Message = bincode::deserialize(&buffer[..n])?;
        match message {
            Message::GetFile { file_hash } => {
                let fs = fs.lock().await;
                if let Some(metadata) = fs.get_file_metadata(&file_hash) {
                    let response = Message::FileMetadata {
                        metadata: metadata.clone(),
                    };
                    let serialized = bincode::serialize(&response)?;
                    socket.write_all(&serialized)?;
                }
            }
            Message::GetChunk { chunk_hash } => {
                let fs = fs.lock().await;
                if let Some(chunk_data) = fs.get_chunk(&chunk_hash).await {
                    let response = Message::ChunkData { chunk: chunk_data };
                    let serialized = bincode::serialize(&response)?;
                    socket.write_all(&serialized)?;
                }
            }
            Message::ListFiles => {
                let fs = fs.lock().await;
                let files = fs.list_files();
                let response = Message::FileList { files };
                let serialized = bincode::serialize(&response)?;
                socket.write_all(&serialized)?;
            }
            Message::AddPeer { peer_addr } => {
                let message = String::from("add peer request recieved");
                let serialized = bincode::serialize(&message)?;
                socket.write_all(&serialized)?;
                let mut peers = known_peers.lock().await;
                if !peers.contains(&peer_addr) {
                    peers.push(peer_addr);
                    // Broadcast the new peer to other nodes
                    NetworkNode::broadcast_add_peer(&*peers, peer_addr).await?;
                }
            }
            _ => {
                let response = Message::Error {
                    message: "Unsupported operation".to_string(),
                };
                let serialized = bincode::serialize(&response)?;
                socket.write_all(&serialized)?;
            }
        }
        Ok(())
    }

    pub async fn add_peer(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.lock().await;

        if addr != self.addr && !peers.contains(&addr) {
            peers.push(addr);
            // broadcast to all the nodes that a new peer has been added
            for &peer in peers.iter() {
                if peer != addr && peer != self.addr {
                    if let Err(e) = Self::send_add_peer(peer, addr).await {
                        eprintln!("Failed to notify peer {}: {}", peer, e);
                    }
                }
            }
        }
    }

    async fn broadcast_add_peer(
        known_peers: &Vec<SocketAddr>,
        new_peer: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        for &peer_addr in known_peers {
            if peer_addr != new_peer {
                if let Err(e) = Self::send_add_peer(peer_addr, new_peer).await {
                    eprintln!("Failed to notify peer {}: {}", peer_addr, e);
                }
            }
        }
        Ok(())
    }

    async fn send_add_peer(
        peer_addr: SocketAddr,
        new_peer: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(peer_addr)?;
        let message = Message::AddPeer {
            peer_addr: new_peer,
        };
        let serialized = bincode::serialize(&message)?;
        stream.write_all(&serialized)?;
        Ok(())
    }

    pub async fn get_file(&self, file_hash: [u8; 32]) -> Result<(String, Vec<u8>), Box<dyn Error>> {
        // println!("inside get_file function");
        let fs = self.fs.lock().await;
        // println!("fs_locked");
        if let Some(metadata) = fs.get_file_metadata(&file_hash) {
            let file_name = metadata.name.clone();
            let chunk_hashes = metadata.chunk_hashes.clone();
            let mut file_data = Vec::with_capacity(metadata.total_size);
            drop(fs);

            for chunk_hash in chunk_hashes {
                let chunk = self.get_chunk_from_network(&chunk_hash).await?;
                file_data.extend_from_slice(&chunk);
            }

            Ok((file_name, file_data))
        } else {
            self.request_file_from_peers(file_hash).await
        }
    }

    async fn get_chunk_from_network(
        &self,
        chunk_hash: &[u8; 32],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        // First try local storage
        let fs = self.fs.lock().await;
        if let Some(chunk) = fs.get_chunk(chunk_hash).await {
            return Ok(chunk);
        }
        drop(fs);

        // Then try known locations
        let locations = self.chunk_locations.lock().await;
        if let Some(peers) = locations.get(chunk_hash) {
            for &peer_addr in peers {
                if let Ok(chunk) = self.request_chunk_from_peer(chunk_hash, peer_addr).await {
                    return Ok(chunk);
                }
            }
        }
        drop(locations);

        // Finally, broadcast request to all peers
        self.broadcast_chunk_request(chunk_hash).await
    }

    async fn request_chunk_from_peer(
        &self,
        chunk_hash: &[u8; 32],
        peer_addr: SocketAddr,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut stream = TcpStream::connect(peer_addr)?;
        let request = Message::GetChunk {
            chunk_hash: *chunk_hash,
        };
        let serialized = bincode::serialize(&request)?;
        stream.write_all(&serialized)?;

        let mut buffer = vec![0; 1024 * 1024];
        let n = stream.read(&mut buffer)?;

        match bincode::deserialize(&buffer[..n])? {
            Message::ChunkData { chunk } => Ok(chunk),
            _ => Err("Unexpected response".into()),
        }
    }

    async fn broadcast_chunk_request(
        &self,
        chunk_hash: &[u8; 32],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let peers = self.known_peers.lock().await;
        for &peer_addr in peers.iter() {
            if let Ok(chunk) = self.request_chunk_from_peer(chunk_hash, peer_addr).await {
                return Ok(chunk);
            }
        }
        Err("Chunk not found in network".into())
    }

    async fn request_file_from_peers(
        &self,
        file_hash: [u8; 32],
    ) -> Result<(String, Vec<u8>), Box<dyn Error>> {
        println!("inside request ffrom peers");
        let peers = self.known_peers.lock().await;
        println!("{:?}", peers);
        for &peer_addr in peers.iter() {
            let mut stream = TcpStream::connect(peer_addr)?;
            let request = Message::GetFile { file_hash };
            let serialized = bincode::serialize(&request)?;
            stream.write_all(&serialized)?;

            let mut buffer = vec![0; 1024 * 1024];
            let n = stream.read(&mut buffer)?;

            match bincode::deserialize(&buffer[..n])? {
                Message::FileMetadata { metadata } => {
                    // Got metadata, now fetch chunks
                    let mut file_data = Vec::with_capacity(metadata.total_size);
                    for chunk_hash in &metadata.chunk_hashes {
                        let chunk = self.get_chunk_from_network(chunk_hash).await?;
                        file_data.extend_from_slice(&chunk);
                    }
                    return Ok((metadata.name, file_data));
                }
                _ => continue,
            }
        }
        Err("File not found in network".into())
    }

    pub async fn get_filesystem(&self) -> Arc<Mutex<FileSystem>> {
        return self.fs.clone();
    }

    pub async fn get_peers(&self) -> Arc<Mutex<Vec<SocketAddr>>> {
        return self.known_peers.clone();
    }

    pub async fn broadcast_new_file(&self, file_hash: [u8; 32]) -> Result<(), Box<dyn Error>> {
        let peers = self.known_peers.lock().await;
        for &peer_addr in peers.iter() {
            if let Err(e) = self.notify_peer_new_file(peer_addr, file_hash).await {
                eprintln!("Failed to notify peer {}: {}", peer_addr, e);
            }
        }
        Ok(())
    }

    async fn notify_peer_new_file(
        &self,
        peer_addr: SocketAddr,
        file_hash: [u8; 32],
    ) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(peer_addr)?;
        let message = Message::NewFile { file_hash };
        let serialized = bincode::serialize(&message)?;
        stream.write_all(&serialized)?;
        Ok(())
    }
}
