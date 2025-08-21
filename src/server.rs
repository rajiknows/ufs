use crate::dht::Peer;
use crate::node::Node;
use crate::storage_proto::{
    peer_service_server::{PeerService, PeerServiceServer},
    FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse, GetChunkRequest,
    GetChunkResponse, GetFileMetadataRequest, GetFileMetadataResponse, InitiateUploadRequest,
    InitiateUploadResponse, PeerMessage, PingRequest, PongResponse, StoreRequest, StoreResponse,
    UploadChunkRequest, UploadChunkResponse,
};
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

pub struct PeerServer {
    node: Arc<Node>,
}

#[tonic::async_trait]
impl PeerService for PeerServer {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PongResponse>, Status> {
        let remote_peer = request.into_inner().peer.unwrap();
        let peer = Peer {
            node_id: remote_peer.node_id.try_into().unwrap(),
            address: remote_peer.address,
        };
        self.node.routing_table.lock().await.add_peer(peer);
        let response = PongResponse {
            node_id: self.node.id.to_vec(),
        };
        Ok(Response::new(response))
    }

    async fn store(
        &self,
        request: Request<StoreRequest>,
    ) -> Result<Response<StoreResponse>, Status> {
        let req = request.into_inner();
        self.node.storage.store_value(&req.key, &req.value);
        Ok(Response::new(StoreResponse { success: true }))
    }

    async fn find_node(
        &self,
        request: Request<FindNodeRequest>,
    ) -> Result<Response<FindNodeResponse>, Status> {
        let req = request.into_inner();
        let target_id: [u8; 32] = req.target_id.try_into().unwrap();
        let peers = self
            .node
            .routing_table
            .lock()
            .await
            .find_closest_peers(&target_id);
        let peer_messages = peers.into_iter().map(Peer::into).collect();
        Ok(Response::new(FindNodeResponse {
            peers: peer_messages,
        }))
    }

    async fn find_value(
        &self,
        request: Request<FindValueRequest>,
    ) -> Result<Response<FindValueResponse>, Status> {
        let req = request.into_inner();
        let key = req.key;

        if let Some(value) = self.node.storage.get_value(&key) {
            Ok(Response::new(FindValueResponse {
                result: Some(crate::storage_proto::find_value_response::Result::Value(
                    value,
                )),
            }))
        } else {
            let target_id: [u8; 32] = key.try_into().unwrap();
            let peers = self
                .node
                .routing_table
                .lock()
                .await
                .find_closest_peers(&target_id);
            let peer_messages = peers.into_iter().map(Peer::into).collect();
            Ok(Response::new(FindValueResponse {
                result: Some(
                    crate::storage_proto::find_value_response::Result::ClosestPeers(
                        FindNodeResponse {
                            peers: peer_messages,
                        },
                    ),
                ),
            }))
        }
    }

    /// Retrieves a file chunk from local storage.
    async fn get_chunk(
        &self,
        request: Request<GetChunkRequest>,
    ) -> Result<Response<GetChunkResponse>, Status> {
        let chunk_hash = request.into_inner().chunk_hash;
        log::info!("Received request for chunk {}", hex::encode(&chunk_hash));

        match self.node.get_chunk(&chunk_hash) {
            Some(chunk_data) => Ok(Response::new(GetChunkResponse { chunk_data })),
            None => Err(Status::not_found("Chunk not found")),
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

        match self.node.get_metadata(&file_hash) {
            Some(metadata) => {
                let serialized_metadata = bincode::serialize(&metadata).unwrap();
                Ok(Response::new(GetFileMetadataResponse {
                    metadata: serialized_metadata,
                }))
            }
            None => Err(Status::not_found("Metadata not found")),
        }
    }

    /// Stores a replica of a chunk sent from another peer.

    async fn list_peers(
        &self,
        _request: Request<crate::storage_proto::ListPeersRequest>,
    ) -> Result<Response<crate::storage_proto::ListPeersResponse>, Status> {
        let peers = self
            .node
            .routing_table
            .lock()
            .await
            .buckets
            .iter()
            .flatten()
            .map(|p| p.address.clone())
            .collect();
        Ok(Response::new(crate::storage_proto::ListPeersResponse {
            peers,
        }))
    }

    async fn list_files(
        &self,
        _request: Request<crate::storage_proto::ListFilesRequest>,
    ) -> Result<Response<crate::storage_proto::ListFilesResponse>, Status> {
        let files = self.node.get_all_metadata();
        let mut proto_files = Vec::new();
        for file in files {
            proto_files.push(file.into());
        }
        Ok(Response::new(crate::storage_proto::ListFilesResponse {
            files: proto_files,
        }))
    }

    async fn initiate_upload(
        &self,
        request: Request<InitiateUploadRequest>,
    ) -> Result<Response<InitiateUploadResponse>, Status> {
        let req = request.into_inner();
        log::info!(
            "Received request to initiate upload for file {}",
            hex::encode(&req.file_hash)
        );

        let metadata = req.metadata.unwrap();

        self.node.store_metadata(
            &req.file_hash,
            &crate::storage::FileInfo {
                name: metadata.name,
                size: metadata.size,
                chunk_hashes: metadata.chunk_hashes,
            },
        );
        Ok(Response::new(InitiateUploadResponse { success: true }))
    }

    async fn upload_chunk(
        &self,
        request: Request<UploadChunkRequest>,
    ) -> Result<Response<UploadChunkResponse>, Status> {
        let req = request.into_inner();
        log::info!(
            "Received request to upload chunk {}",
            hex::encode(&req.chunk_hash)
        );

        self.node.store_chunk(&req.chunk_hash, &req.chunk_data);
        Ok(Response::new(UploadChunkResponse { success: true }))
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

impl From<Peer> for PeerMessage {
    fn from(peer: Peer) -> Self {
        Self {
            node_id: peer.node_id.to_vec(),
            address: peer.address,
        }
    }
}

/// Initializes and runs the gRPC server.
pub async fn start_server(
    port: u16,
    bootstrap_peer: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("[::1]:{}", port).parse()?;
    let node_addr = format!("http://[::1]:{}", port);
    let node = Arc::new(Node::new(&node_addr)?);

    let peer_server = PeerServer { node: node.clone() };

    log::info!("Server listening on {}", addr);

    // Start the node's background tasks (bootstrapping)
    node.start(bootstrap_peer).await?;

    // Start the gRPC server
    Server::builder()
        .add_service(PeerServiceServer::new(peer_server))
        .serve(addr)
        .await?;

    Ok(())
}
