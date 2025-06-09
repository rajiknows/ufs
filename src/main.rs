use std::sync::Arc;

mod cli;
mod fs;
mod grpc;
mod network;
use grpc::filesystem::file_system_service_server::FileSystemService;
use grpc::filesystem::{self};
use grpc::FileSystemServer;
use network::NetworkNode;

use filesystem::file_system_service_server::FileSystemServiceServer;
mod kv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let node = Arc::new(NetworkNode::new(addr));
    let service = FileSystemServer { node };
    node.start().await;
    println!("gRPC server listening on {}", addr);
    tonic::transport::Server::builder()
        .add_service(FileSystemServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
