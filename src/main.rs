use axum::{
    extract::{Json, Path, State},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::Mutex};

mod fs;
mod network;

use network::NetworkNode;

#[tokio::main]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();

    let node = Arc::new(Mutex::new(NetworkNode::new(addr)));

    // Start the DFS client in a separate task
    let node_clone = Arc::clone(&node);
    tokio::spawn(async move {
        let mut locked_node = node_clone.lock().await;
        if let Err(e) = locked_node.start().await {
            eprintln!("Error starting DFS client: {}", e);
        }
    });

    // Build the Axum app
    let app = Router::new()
        .route("/files", get(list_files).post(upload_file))
        .route("/files/:hash", get(download_file).delete(delete_file))
        .route("/peers", get(list_peers).post(add_peer))
        .route("/uptime", get(get_uptime))
        .with_state(node.clone());

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port");

    axum::serve(listener, app).await.unwrap();
}

async fn list_files(State(node): State<Arc<Mutex<NetworkNode>>>) -> Json<Vec<(String, String)>> {
    let locked_node = node.lock().await;
    let files = locked_node.list_files().await;
    Json(
        files
            .into_iter()
            .map(|(hash, name)| (hex::encode(hash), name))
            .collect(),
    )
}

#[derive(Deserialize)]
struct FileUpload {
    path: String,
}

async fn upload_file(
    State(node): State<Arc<Mutex<NetworkNode>>>,
    Json(payload): Json<FileUpload>,
) -> Json<String> {
    let locked_node = node.lock().await;
    let file_hash = locked_node
        .get_filesystem()
        .await
        .lock()
        .await
        .add_file_from_path(&payload.path)
        .await;
    Json(hex::encode(file_hash))
}

async fn download_file(
    Path(hash): Path<String>,
    State(node): State<Arc<Mutex<NetworkNode>>>,
) -> Json<Option<Vec<u8>>> {
    let locked_node = node.lock().await;
    let hash_bytes = hex::decode(hash).unwrap();
    match locked_node.get_file(hash_bytes.try_into().unwrap()).await {
        Ok((_, data)) => Json(Some(data)),
        Err(_) => Json(None),
    }
}

async fn delete_file(Path(_hash): Path<String>) -> &'static str {
    // Not implemented in the backend
    "Delete endpoint not implemented"
}

async fn list_peers(State(node): State<Arc<Mutex<NetworkNode>>>) -> Json<Vec<String>> {
    let locked_node = node.lock().await;
    Json(
        locked_node
            .get_peers()
            .await
            .into_iter()
            .map(|addr| addr.to_string())
            .collect(),
    )
}

#[derive(Deserialize)]
struct PeerAdd {
    addr: String,
}

async fn add_peer(
    State(node): State<Arc<Mutex<NetworkNode>>>,
    Json(payload): Json<PeerAdd>,
) -> &'static str {
    let locked_node = node.lock().await;
    if let Ok(peer_addr) = payload.addr.parse() {
        locked_node.add_peer(peer_addr).await;
        "Peer added successfully"
    } else {
        "Invalid address"
    }
}

async fn get_uptime(State(node): State<Arc<Mutex<NetworkNode>>>) -> Json<u32> {
    let locked_node = node.lock().await;
    Json(locked_node.get_uptime().await.unwrap())
}
