use futures::future::ok;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::fs::{FileInfo, FileSystem};
use crate::grpc::filesystem::file_system_service_client::FileSystemServiceClient;
use crate::grpc::filesystem::{
    AddPeerRequest, GetChunkRequest, GetFileRequest, NewFileRequest, SyncRequest,
};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    GetFile { file_hash: [u8; 32] },
    GetChunk { chunk_hash: [u8; 32] },
    FileMetadata { metadata: FileInfo },
    ChunkData { chunk: Vec<u8> },
    ListFiles,
    FileList { files: Vec<([u8; 32], String)> },
    Error { message: String },
    NewFile { file_hash: [u8; 32] },
    DeleteFile { file_hash: [u8; 32] },
    AddPeer { peer_addr: SocketAddr },
    SyncRequest,
    SyncResponse { files: Vec<FileInfo> },
    Ping,
    Pong,
}

#[derive(Debug)]
struct NetworkNodeInner {
    addr: SocketAddr,
    fs: Arc<Mutex<FileSystem>>,
    known_peers: Arc<Mutex<Vec<SocketAddr>>>,
    dht: Arc<Mutex<HashMap<[u8; 32], Vec<SocketAddr>>>>,
    uptime: Arc<Mutex<u32>>,
    is_started: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct NetworkNode {
    inner: Arc<NetworkNodeInner>,
}

impl NetworkNode {
    async fn get_client(
        &self,
        peer_addr: SocketAddr,
    ) -> Result<FileSystemServiceClient<Channel>, Box<dyn Error>> {
        Ok(FileSystemServiceClient::connect(format!("http://{}", peer_addr)).await?)
    }

    pub fn new(addr: SocketAddr) -> Self {
        NetworkNode {
            inner: Arc::new(NetworkNodeInner {
                addr,
                fs: Arc::new(Mutex::new(FileSystem::new())),
                known_peers: Arc::new(Mutex::new(Vec::new())),
                dht: Arc::new(Mutex::new(HashMap::new())),
                uptime: Arc::new(Mutex::new(0)),
                is_started: Arc::new(Mutex::new(false)),
            }),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        // Check if already started
        let mut is_started = self.inner.is_started.lock().await;
        if *is_started {
            return Ok(());
        }
        *is_started = true;
        drop(is_started); // Release the lock

        // let listener = TcpListener::bind(self.inner.addr)?;
        // println!("Listening on {}", self.inner.addr);

        let uptime = self.inner.uptime.clone();

        // Start uptime counter
        tokio::spawn(async move {
            loop {
                {
                    let mut uptime = uptime.lock().await;
                    *uptime += 1;
                }
                sleep(Duration::from_secs(1)).await;
            }
        });
        return Ok(());
    }

    pub async fn get_uptime(&self) -> Result<u32, ()> {
        let uptime = self.inner.uptime.lock().await;
        Ok(uptime.clone())
    }

    pub async fn list_files(&self) -> Vec<([u8; 32], String)> {
        let fs = self.inner.fs.lock().await;
        let files = fs.list_files();
        files
    }

    async fn sync_with_peer(&self, peer_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut client = self.get_client(peer_addr).await?;
        let request = tonic::Request::new(SyncRequest {});
        let response = client.sync(request).await?;
        let files = response.into_inner().files;
        let mut fs = self.inner.fs.lock().await;
        for file_info in files {
            fs.add_file_metadata(file_info.hash.try_into()?, file_info.into());
        }
        Ok(())
    }

    pub async fn add_peer(&self, addr: SocketAddr) {
        let mut peers = self.inner.known_peers.lock().await;
        if addr != self.inner.addr && !peers.contains(&addr) {
            peers.push(addr);
            drop(peers);

            if let Err(e) = self.send_add_peer(addr, self.inner.addr).await {
                eprintln!("Failed to notify peer {}: {}", addr, e);
            }

            if let Err(e) = self.sync_with_peer(addr).await {
                eprintln!("Failed to sync with peer {}: {}", addr, e);
            }
        }
    }

    pub async fn get_filesystem(&self) -> Arc<Mutex<FileSystem>> {
        return self.inner.fs.clone();
    }

    pub async fn get_peers(&self) -> Vec<SocketAddr> {
        let peers = self.inner.known_peers.lock().await;
        peers.clone()
    }

    pub async fn get_file(&self, file_hash: [u8; 32]) -> Result<(String, Vec<u8>), Box<dyn Error>> {
        let fs = self.inner.fs.lock().await;
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
        let fs = self.inner.fs.lock().await;
        if let Some(chunk) = fs.get_chunk(chunk_hash).await {
            return Ok(chunk);
        }
        drop(fs);

        // Then try known locations
        let locations = self.inner.dht.lock().await;
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
        let mut client = self.get_client(peer_addr).await?;
        let request = tonic::Request::new(GetChunkRequest {
            chunk_hash: chunk_hash.to_vec(),
        });
        let response = client.get_chunk(request).await?;
        Ok(response.into_inner().chunk_data)
    }

    async fn broadcast_chunk_request(
        &self,
        chunk_hash: &[u8; 32],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let peers = self.inner.known_peers.lock().await;
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
        let peers = self.inner.known_peers.lock().await;
        for &peer_addr in peers.iter() {
            let mut client = self.get_client(peer_addr).await?;
            let request = tonic::Request::new(GetFileRequest {
                file_hash: file_hash.to_vec(),
            });
            match client.get_file(request).await {
                Ok(response) => {
                    let metadata = response.into_inner().metadata.ok_or("No metadata")?;
                    let mut file_data = Vec::with_capacity(metadata.total_size as usize);
                    for chunk_hash in &metadata.hash {
                        let chunk = self.get_chunk_from_network(&chunk_hash.try_into()?).await?;
                        file_data.extend_from_slice(&chunk);
                    }
                    return Ok((metadata.name, file_data));
                }
                Err(_) => continue,
            }
        }
        Err("File not found in network".into())
    }

    pub async fn broadcast_new_file(&self, file_hash: [u8; 32]) -> Result<(), Box<dyn Error>> {
        let peers = self.inner.known_peers.lock().await;
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
        let mut client = self.get_client(peer_addr).await?;
        let request = tonic::Request::new(NewFileRequest {
            file_hash: file_hash.to_vec(),
        });
        client.notify_new_file(request).await?;
        Ok(())
    }

    async fn send_add_peer(
        &self,
        peer_addr: SocketAddr,
        new_peer: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        let mut client = self.get_client(peer_addr).await?;
        let request = tonic::Request::new(AddPeerRequest {
            peer_address: new_peer.to_string(),
        });
        client.add_peer(request).await?;
        Ok(())
    }
    //
    //pub async fn ping_self_nodes(&self) {
    //    let peers = self.inner.known_peers.lock().await;
    //    for &peer_addr in peers.iter() {
    //        if let Err(e) = ping_peer(peer_addr).await {
    //            eprintln!("Failed to ping peer {}: {}", peer_addr, e);
    //        }
    //    }
    //}
}
