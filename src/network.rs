use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

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
    SyncRequest,
    SyncResponse { files: Vec<FileInfo> },
    Ping,
    Pong,
}

#[derive(Debug, Clone)]
pub struct NetworkNode {
    addr: SocketAddr,
    fs: Arc<Mutex<FileSystem>>,
    known_peers: Arc<Mutex<Vec<SocketAddr>>>,
    dht: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
    uptime: Arc<Mutex<u32>>,
}

impl NetworkNode {
    pub fn new(addr: SocketAddr) -> Self {
        NetworkNode {
            fs: Arc::new(Mutex::new(FileSystem::new())),
            known_peers: Arc::new(Mutex::new(Vec::new())),
            dht: Arc::new(Mutex::new(HashMap::new())),
            addr,
            uptime: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(self.addr)?;
        println!("Listening on {}", self.addr);

        let uptime = self.uptime.clone();

        tokio::spawn(async move {
            loop {
                {
                    let mut uptime = uptime.lock().await;
                    *uptime += 1;
                }
                sleep(Duration::from_secs(1)).await;
            }
        });

        loop {
            let (socket, peer_addr) = listener.accept()?;
            println!("New connection from {}", peer_addr);

            let fs = self.fs.clone();
            let dht = self.dht.clone();
            let known_peers = self.known_peers.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, fs, dht, known_peers).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
    pub async fn get_uptime(&self) -> Result<u32, ()> {
        let uptime = self.uptime.lock().await;
        Ok(uptime.clone())
    }

    pub async fn list_files(&self) -> Vec<([u8; 32], String)> {
        let fs = self.fs.lock().await;
        let files = fs.list_files();
        files
    }

    async fn handle_connection(
        mut socket: TcpStream,
        fs: Arc<Mutex<FileSystem>>,
        _dht: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
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
                } else {
                    let response = Message::Error {
                        message: "File not found".to_string(),
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
                } else {
                    let response = Message::Error {
                        message: "Chunk not found".to_string(),
                    };
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

            Message::NewFile { file_hash } => {
                // When a new file is announced, request its metadata and chunks
                let peer_addr = socket.peer_addr()?;
                let mut stream = TcpStream::connect(peer_addr)?;
                let request = Message::GetFile { file_hash };
                let serialized = bincode::serialize(&request)?;
                stream.write_all(&serialized)?;

                let mut buffer = vec![0; 1024 * 1024];
                let n = stream.read(&mut buffer)?;

                match bincode::deserialize(&buffer[..n])? {
                    Message::FileMetadata { metadata } => {
                        let mut fs = fs.lock().await;
                        fs.add_file_metadata(file_hash, metadata);
                    }
                    _ => {}
                }
            }

            Message::AddPeer { peer_addr } => {
                let mut peers = known_peers.lock().await;
                if !peers.contains(&peer_addr) {
                    peers.push(peer_addr);
                    // Sync files with the new peer
                    drop(peers); // Release the lock before making network calls
                    if let Err(e) = Self::sync_with_peer(peer_addr, fs.clone()).await {
                        eprintln!("Failed to sync with peer {}: {}", peer_addr, e);
                    }
                }

                let response = Message::Error {
                    message: "Peer added successfully".to_string(),
                };
                let serialized = bincode::serialize(&response)?;
                socket.write_all(&serialized)?;
            }

            Message::SyncRequest => {
                let fs = fs.lock().await;
                let mut files = Vec::new();
                for (hash, _) in fs.list_files() {
                    if let Some(metadata) = fs.get_file_metadata(&hash) {
                        files.push(metadata.clone());
                    }
                }
                let response = Message::SyncResponse { files };
                let serialized = bincode::serialize(&response)?;
                socket.write_all(&serialized)?;
            }
            Message::Ping => {
                let response = Message::Pong;
                let serialized = bincode::serialize(&response)?;
                socket.write_all(&serialized)?;
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

    async fn sync_with_peer(
        peer_addr: SocketAddr,
        fs: Arc<Mutex<FileSystem>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(peer_addr)?;
        let request = Message::SyncRequest;
        let serialized = bincode::serialize(&request)?;
        stream.write_all(&serialized)?;

        let mut buffer = vec![0; 1024 * 1024];
        let n = stream.read(&mut buffer)?;

        match bincode::deserialize(&buffer[..n])? {
            Message::SyncResponse { files } => {
                let mut fs = fs.lock().await;
                for file_info in files {
                    fs.add_file_metadata(file_info.filehash, file_info);
                }
                Ok(())
            }
            _ => Err("Unexpected response during sync".into()),
        }
    }

    pub async fn add_peer(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.lock().await;
        if addr != self.addr && !peers.contains(&addr) {
            peers.push(addr);
            drop(peers);

            // Send AddPeer message to the new peer
            if let Err(e) = self.send_add_peer(addr, self.addr).await {
                eprintln!("Failed to notify peer {}: {}", addr, e);
            }

            // Sync files with the new peer
            if let Err(e) = Self::sync_with_peer(addr, self.fs.clone()).await {
                eprintln!("Failed to sync with peer {}: {}", addr, e);
            }
        }
    }

    pub async fn get_filesystem(&self) -> Arc<Mutex<FileSystem>> {
        return self.fs.clone();
    }

    pub async fn get_peers(&self) -> Vec<SocketAddr> {
        let peers = self.known_peers.lock().await;
        peers.clone()
    }

    pub async fn get_file(&self, file_hash: [u8; 32]) -> Result<(String, Vec<u8>), Box<dyn Error>> {
        let fs = self.fs.lock().await;
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
        let locations = self.dht.lock().await;
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
        let peers = self.known_peers.lock().await;
        for &peer_addr in peers.iter() {
            let mut stream = TcpStream::connect(peer_addr)?;
            let request = Message::GetFile { file_hash };
            let serialized = bincode::serialize(&request)?;
            stream.write_all(&serialized)?;

            let mut buffer = vec![0; 1024 * 1024];
            let n = stream.read(&mut buffer)?;

            match bincode::deserialize(&buffer[..n])? {
                Message::FileMetadata { metadata } => {
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

    async fn send_add_peer(
        &self,
        peer_addr: SocketAddr,
        new_peer: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(peer_addr)?;
        let message = Message::AddPeer {
            peer_addr: new_peer,
        };
        let serialized = bincode::serialize(&message)?;
        stream.write_all(&serialized)?;
        let mut buffer = vec![0; 1024];
        stream.read(&mut buffer)?; // Wait for acknowledgment
        Ok(())
    }

    pub async fn ping_self_nodes(&self) {
        let peers = self.known_peers.lock().await;
        for &peer_addr in peers.iter() {
            if let Err(e) = ping_peer(peer_addr).await {
                eprintln!("Failed to ping peer {}: {}", peer_addr, e);
            }
        }
    }
}

pub async fn ping_peer(peer_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(peer_addr)?;
    let message = Message::Ping;
    let serialized = bincode::serialize(&message)?;
    stream.write_all(&serialized)?;
    let mut buffer = vec![0; 1024];
    stream.read(&mut buffer)?;
    if let Message::Pong = bincode::deserialize(&buffer)? {
        println!("peer active : {peer_addr}");
        Ok(())
    } else {
        Err("Unexpected response".into())
    }
}
