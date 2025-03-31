use std::io::{Read, Write};
use std::net::TcpStream;
use std::{error::Error, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

use crate::network::NetworkNode;
use crate::{fs::FileSystem, network::Message};

pub async fn handle_connection(
    mut socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
    known_peers: Arc<Mutex<Vec<SocketAddr>>>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
    let n = socket.read(&mut buffer)?;

    let message: Message = bincode::deserialize(&buffer[..n])?;
    match message {
        Message::GetFile { file_hash } => new_file_handler(socket, fs, file_hash).await?,
        Message::GetChunk { chunk_hash } => get_chunk_handler(socket, fs, chunk_hash).await?,
        Message::ListFiles => list_file_handler(socket, fs).await?,
        Message::NewFile { file_hash } => new_file_handler(socket, fs, file_hash).await?,
        Message::DeleteFile { file_hash } => delete_file_handler(socket, fs, file_hash).await?,
        Message::AddPeer { peer_addr } => {
            add_peer_handler(socket, known_peers, fs, peer_addr).await?
        }
        Message::SyncRequest => sync_request_handler(socket, fs).await?,
        Message::Ping => ping_handler(socket).await?,
        _ => unsupported_operation_handler(socket).await?,
    }
    Ok(())
}

pub async fn new_file_handler(
    socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
    file_hash: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let peer_addr = socket.peer_addr()?;
    let mut stream = TcpStream::connect(peer_addr)?;
    let request = Message::GetFile { file_hash };
    let serialized = bincode::serialize(&request)?;
    stream.write_all(&serialized)?;

    let mut buffer = vec![0; 1024 * 1024];
    let n = stream.read(&mut buffer)?;

    if let Message::FileMetadata { metadata } = bincode::deserialize(&buffer[..n])? {
        let mut fs = fs.lock().await;
        fs.add_file_metadata(file_hash, metadata);
    }
    Ok(())
}

pub async fn delete_file_handler(
    mut _socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
    file_hash: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let mut fs = fs.lock().await;
    fs.delete_file(file_hash).await;
    Ok(())
}

pub async fn add_peer_handler(
    mut socket: TcpStream,
    known_peers: Arc<Mutex<Vec<SocketAddr>>>,
    fs: Arc<Mutex<FileSystem>>,
    peer_addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let mut peers = known_peers.lock().await;
    if !peers.contains(&peer_addr) {
        peers.push(peer_addr);
        drop(peers);
        if let Err(e) = NetworkNode::sync_with_peer(peer_addr, fs).await {
            eprintln!("Failed to sync with peer {}: {}", peer_addr, e);
        }
    }

    let response = Message::Error {
        message: "Peer added successfully".to_string(),
    };
    let serialized = bincode::serialize(&response)?;
    socket.write_all(&serialized)?;
    Ok(())
}

pub async fn sync_request_handler(
    mut socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

pub async fn ping_handler(mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
    let response = Message::Pong;
    let serialized = bincode::serialize(&response)?;
    socket.write_all(&serialized)?;
    Ok(())
}

pub async fn unsupported_operation_handler(mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
    let response = Message::Error {
        message: "Unsupported operation".to_string(),
    };
    let serialized = bincode::serialize(&response)?;
    socket.write_all(&serialized)?;
    Ok(())
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

async fn get_chunk_handler(
    mut socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
    chunk_hash: [u8; 32],
) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

async fn list_file_handler(
    mut socket: TcpStream,
    fs: Arc<Mutex<FileSystem>>,
) -> Result<(), Box<dyn Error>> {
    let fs = fs.lock().await;
    let file_list: Vec<_> = fs.list_files().into_iter().collect();
    let response = Message::FileList { files: file_list };
    let serialized = bincode::serialize(&response)?;
    socket.write_all(&serialized)?;
    Ok(())
}
