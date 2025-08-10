use crate::node::Node;
use crate::storage_proto::{
    peer_service_server::{PeerService, PeerServiceServer},
    GetChunkRequest, GetChunkResponse, GetFileMetadataRequest, GetFileMetadataResponse,
    GossipMessage, GossipResponse, PeerRequest, PeerResponse, StoreChunkRequest,
    StoreChunkResponse, StoreFileMetadataRequest, StoreFileMetadataResponse,
};
use std::path::PathBuf;
use std::sync::Arc;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

pub struct PeerServer {
    // The node contains all the application logic and state.
    node: Arc<Node>,
}

#[tonic::async_trait]
impl PeerService for PeerServer {
    /// Handles requests from other peers to exchange peer lists.
    async fn share_peers(
        &self,
        request: Request<PeerRequest>,
    ) -> Result<Response<PeerResponse>, Status> {
        let remote_request = request.into_inner();
        let from_address = remote_request.from_address;
        log::info!("Received SharePeers request from {}", from_address);

        self.node.add_peers(remote_request.known_peers).await;
        self.node.add_peer(from_address).await;

        let known_peers = self.node.get_peers().await;
        let response = PeerResponse { known_peers };
        Ok(Response::new(response))
    }

    async fn store_file_metadata(
        &self,
        request: tonic::Request<StoreFileMetadataRequest>,
    ) -> Result<Response<StoreFileMetadataResponse>, Status> {
        let req = request.into_inner();
        log::info!(
            "Received request to store file metadata for hash {}",
            hex::encode(&req.file_hash)
        );

        let metadata: crate::storage::FileInfo = bincode::deserialize(&req.metadata)
            .map_err(|e| Status::internal(format!("Failed to deserialize metadata: {}", e)))?;

        match self.node.store_metadata(&req.file_hash, metadata).await {
            Ok(_) => Ok(Response::new(StoreFileMetadataResponse { success: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    /// The main entry point for the gossip protocol.
    async fn gossip(
        &self,
        request: Request<GossipMessage>,
    ) -> Result<Response<GossipResponse>, Status> {
        let message = request.into_inner();
        log::info!(
            "Received gossip with {} file hashes",
            message.file_hashes.len()
        );

        let hashes = message.file_hashes;
        self.node.handle_gossip(hashes).await;

        Ok(Response::new(GossipResponse { success: true }))
    }

    /// Retrieves a file chunk from local storage.
    async fn get_chunk(
        &self,
        request: Request<GetChunkRequest>,
    ) -> Result<Response<GetChunkResponse>, Status> {
        let chunk_hash = request.into_inner().chunk_hash;
        log::info!("Received request for chunk {}", hex::encode(&chunk_hash));

        match self.node.get_chunk(&chunk_hash).await {
            Ok(Some(chunk_data)) => Ok(Response::new(GetChunkResponse { chunk_data })),
            Ok(None) => Err(Status::not_found("Chunk not found")),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    /// Retrieves file metadata from local storage.
    async fn get_file_metadata(
        &self,
        request: Request<GetFileMetadataRequest>,
    ) -> Result<Response<GetFileMetadataResponse>, Status> {
        let file_hash = request.into_inner().file_hash;
        log::info!(
            "Received request for metadata for file {}",
            hex::encode(&file_hash)
        );

        match self.node.get_metadata(&file_hash).await {
            Ok(Some(metadata)) => {
                let serialized_metadata = bincode::serialize(&metadata).unwrap();
                Ok(Response::new(GetFileMetadataResponse {
                    metadata: serialized_metadata,
                }))
            }
            Ok(None) => Err(Status::not_found("Metadata not found")),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    /// Stores a replica of a chunk sent from another peer.
    async fn store_chunk(
        &self,
        request: Request<StoreChunkRequest>,
    ) -> Result<Response<StoreChunkResponse>, Status> {
        let req = request.into_inner();
        log::info!(
            "Received request to store chunk {}",
            hex::encode(&req.chunk_hash)
        );

        match self
            .node
            .store_chunk(&req.chunk_hash, &req.chunk_data)
            .await
        {
            Ok(_) => Ok(Response::new(StoreChunkResponse { success: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn list_peers(
        &self,
        _request: Request<crate::storage_proto::ListPeersRequest>,
    ) -> Result<Response<crate::storage_proto::ListPeersResponse>, Status> {
        let peers = self.node.get_peers().await;
        Ok(Response::new(crate::storage_proto::ListPeersResponse {
            peers,
        }))
    }

    async fn list_files(
        &self,
        _request: Request<crate::storage_proto::ListFilesRequest>,
    ) -> Result<Response<crate::storage_proto::ListFilesResponse>, Status> {
        let files = self.node.get_all_metadata().await.unwrap_or_default();
        let mut proto_files = Vec::new();
        for file in files {
            proto_files.push(file.into());
        }
        Ok(Response::new(crate::storage_proto::ListFilesResponse {
            files: proto_files,
        }))
    }
}

impl From<crate::storage::FileInfo> for crate::storage_proto::FileInfo {
    fn from(file_info: crate::storage::FileInfo) -> Self {
        crate::storage_proto::FileInfo {
            name: file_info.name,
            size: file_info.size,
            chunk_hashes: file_info.chunk_hashes,
        }
    }
}

/// Initializes and runs the gRPC server.
pub async fn start_server(
    port: u16,
    bootstrap_peer: Option<String>,
    db_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("[::1]:{}", port).parse()?;
    let node = Arc::new(Node::new(
        &db_path.to_string_lossy(),
        format!("[::1]:{}", port),
    )?);

    let peer_server = PeerServer { node: node.clone() };

    log::info!("Server listening on {}", addr);

    // Start the node's background tasks (gossiping, bootstrapping)
    node.start(bootstrap_peer).await?;

    // Start the gRPC server
    Server::builder()
        .add_service(PeerServiceServer::new(peer_server))
        .serve(addr)
        .await?;

    Ok(())
}
