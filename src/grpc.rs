use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod filesystem {
    tonic::include_proto!("filesystem"); // Include generated code
}

use filesystem::file_system_service_server::FileSystemService;
use filesystem::*;

use crate::network::NetworkNode;

// Assume NetworkNode is your existing type
pub struct FileSystemServer {
    pub node: Arc<NetworkNode>,
}

#[tonic::async_trait]
impl FileSystemService for FileSystemServer {
    async fn list_files(
        &self,
        _: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let files = self.node.filesystem.list_files(); // Adjust based on your API
        let response = ListFilesResponse {
            files: files
                .into_iter()
                .map(|f| FileInfo {
                    name: f.name,
                    hash: f.hash,
                })
                .collect(),
        };
        Ok(Response::new(response))
    }

    async fn upload_file(
        &self,
        request: Request<tonic::Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, Status> {
        let mut stream = request.into_inner();
        let temp_path = format!("/tmp/ufs-upload-{}", Uuid::new_v4());
        let mut file = File::create(&temp_path).await?;
        let mut name = String::new();

        while let Some(req) = stream.message().await? {
            match req.content {
                Some(upload_file_request::Content::Metadata(meta)) => name = meta.name,
                Some(upload_file_request::Content::Chunk(chunk)) => file.write_all(&chunk).await?,
                None => return Err(Status::invalid_argument("Empty request content")),
            }
        }

        file.flush().await?;
        let hash = self.node.upload_file(&name, &temp_path).await?; // Adjust to your upload logic
        tokio::fs::remove_file(&temp_path).await?;
        Ok(Response::new(UploadFileResponse { hash }))
    }

    type DownloadFileStream = mpsc::Receiver<Result<DownloadFileResponse, Status>>;
    async fn download_file(
        &self,
        request: Request<DownloadFileRequest>,
    ) -> Result<Response<Self::DownloadFileStream>, Status> {
        let hash = request.into_inner().hash;
        let temp_path = self.node.download_file(&hash).await?; // Returns path to reassembled file
        let (tx, rx) = tokio::sync::mpsc::channel(4);

        tokio::spawn(async move {
            let mut file = File::open(&temp_path).await?;
            let mut buffer = vec![0u8; 1024 * 1024]; // 1MB chunks
            loop {
                let n = file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                tx.send(Ok(DownloadFileResponse {
                    chunk: buffer[..n].to_vec(),
                }))
                .await?;
            }
            tokio::fs::remove_file(&temp_path).await?;
            Ok::<(), Status>(())
        });

        Ok(Response::new(rx))
    }

    async fn list_peers(
        &self,
        _: Request<ListPeersRequest>,
    ) -> Result<Response<ListPeersResponse>, Status> {
        let peers = self.node.list_peers(); // Adjust based on your API
        Ok(Response::new(ListPeersResponse { peers }))
    }

    async fn add_peer(
        &self,
        request: Request<AddPeerRequest>,
    ) -> Result<Response<AddPeerResponse>, Status> {
        let peer_address = request.into_inner().peer_address;
        let success = self.node.add_peer(&peer_address).is_ok(); // Adjust based on your API
        Ok(Response::new(AddPeerResponse {
            success,
            message: if success {
                "Peer added"
            } else {
                "Failed to add peer"
            }
            .to_string(),
        }))
    }

    async fn get_uptime(
        &self,
        _: Request<GetUptimeRequest>,
    ) -> Result<Response<GetUptimeResponse>, Status> {
        let uptime_seconds = self.node.uptime(); // Adjust based on your API
        Ok(Response::new(GetUptimeResponse { uptime_seconds }))
    }
}
