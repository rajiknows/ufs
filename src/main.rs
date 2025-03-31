use std::sync::Arc;

mod cli;
mod fs;
mod grpc;
mod network;
use grpc::filesystem;
use grpc::FileSystemServer;
use network::NetworkNode;

use filesystem::file_system_service_server::FileSystemServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let node = Arc::new(NetworkNode::new(addr));
    let service = FileSystemServer { node };

    println!("gRPC server listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(FileSystemServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

// use axum::{
//     body::Body,
//     extract::{ConnectInfo, Json, Path, State},
//     http::{self, Request, StatusCode},
//     middleware::{self, Next},
//     response::Response,
//     routing::get,
//     Router,
// };
// use chrono::Local;
// use serde::Deserialize;
// use std::{net::SocketAddr, sync::Arc, time::Instant};
// use tokio::net::TcpListener;
// use tower_http::cors::{Any, CorsLayer};

// mod fs;
// mod handlers;
// mod network;
// use network::NetworkNode;
// mod grpc;

// async fn logging_middleware(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
//     let start_time = Instant::now();
//     let path = request.uri().path().to_owned();
//     let method = request.method().clone();

//     let client_ip = if let Some(forwarded) = request.headers().get("x-forwarded-for") {
//         forwarded
//             .to_str()
//             .map(String::from)
//             .unwrap_or_else(|_| String::from("invalid"))
//     } else {
//         request
//             .extensions()
//             .get::<ConnectInfo<SocketAddr>>()
//             .map(|connect_info| connect_info.0.ip().to_string())
//             .unwrap_or_else(|| String::from("unknown"))
//     };

//     let response = next.run(request).await;
//     let duration = start_time.elapsed();

//     println!(
//         "[{}] {} {} from {} - {:?} - {:?}",
//         Local::now().format("%Y-%m-%d %H:%M:%S"),
//         method,
//         path,
//         client_ip,
//         duration,
//         response.status(),
//     );

//     Ok(response)
// }

// #[tokio::main]
// async fn main() {
//     let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
//     let dfs_addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();

//     // Create the network node
//     let node = Arc::new(NetworkNode::new(dfs_addr));

//     // Clone for the network service
//     let node_clone = Arc::clone(&node);

//     // Start the network service in a separate task
//     tokio::spawn(async move {
//         if let Err(e) = node_clone.start().await {
//             eprintln!("Error starting DFS client: {}", e);
//         }
//     });

//     let cors = CorsLayer::new()
//         .allow_origin(Any)
//         .allow_methods([
//             http::Method::GET,
//             http::Method::POST,
//             http::Method::DELETE,
//             http::Method::OPTIONS,
//         ])
//         .allow_headers(Any);

//     let app = Router::new()
//         .route("/connect", get(connect))
//         .route("/files", get(list_files).post(upload_file))
//         .route("/files/:hash", get(download_file).delete(delete_file))
//         .route("/peers", get(list_peers).post(add_peer))
//         .route("/uptime", get(get_uptime))
//         .layer(cors)
//         .layer(middleware::from_fn(logging_middleware))
//         .with_state(node);

//     println!("Server starting on {}", addr);

//     let listener = TcpListener::bind(addr)
//         .await
//         .expect("Failed to bind to port");

//     axum::serve(
//         listener,
//         app.into_make_service_with_connect_info::<SocketAddr>(),
//     )
//     .await
//     .unwrap();
// }

// async fn connect() -> &'static str {
//     "Connected"
// }

// async fn list_files(State(node): State<Arc<NetworkNode>>) -> Json<Vec<(String, String)>> {
//     let files = node.list_files().await;
//     Json(
//         files
//             .into_iter()
//             .map(|(hash, name)| (hex::encode(hash), name))
//             .collect(),
//     )
// }

// #[derive(Deserialize)]
// struct FileUpload {
//     path: String,
// }

// async fn upload_file(
//     State(node): State<Arc<NetworkNode>>,
//     Json(payload): Json<FileUpload>,
// ) -> Json<String> {
//     // Json("file uploaded".to_string())
//     let fs = node.get_filesystem().await;
//     let file_hash = fs.lock().await.add_file_from_path(&payload.path).await;
//     Json(hex::encode(file_hash))
// }

// async fn download_file(
//     Path(hash): Path<String>,
//     State(node): State<Arc<NetworkNode>>,
// ) -> Json<Option<Vec<u8>>> {
//     let hash_bytes = hex::decode(hash).unwrap();
//     match node.get_file(hash_bytes.try_into().unwrap()).await {
//         Ok((_, data)) => Json(Some(data)),
//         Err(_) => Json(None),
//     }
// }

// async fn delete_file(
//     Path(hash): Path<String>,
//     State(node): State<Arc<NetworkNode>>,
// ) -> &'static str {
//     let hash_bytes: [u8; 32] = hex::decode(hash).unwrap().try_into().unwrap();
//     node.get_filesystem()
//         .await
//         .lock()
//         .await
//         .delete_file(hash_bytes)
//         .await;
//     "File deleted successfully"
// }

// async fn list_peers(State(node): State<Arc<NetworkNode>>) -> Json<Vec<String>> {
//     let peers = node.get_peers().await;
//     let peers: Vec<String> = peers.into_iter().map(|addr| addr.to_string()).collect();

//     if peers.is_empty() {
//         Json(vec!["No peers".to_string()])
//     } else {
//         Json(peers)
//     }
// }

// #[derive(Deserialize)]
// struct PeerAdd {
//     addr: String,
// }

// async fn add_peer(
//     State(node): State<Arc<NetworkNode>>,
//     Json(payload): Json<PeerAdd>,
// ) -> &'static str {
//     if let Ok(peer_addr) = payload.addr.parse() {
//         node.add_peer(peer_addr).await;
//         "Peer added successfully"
//     } else {
//         "Invalid address"
//     }
// }

// async fn get_uptime(State(node): State<Arc<NetworkNode>>) -> Json<u32> {
//     Json(node.get_uptime().await.unwrap())
// }
