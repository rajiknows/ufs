use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod filesystem {
    tonic::include_proto!("filesystem");
}

use filesystem::file_system_service_server::FileSystemService;
use filesystem::*;

use crate::network::NetworkNode;

pub struct FileSystemServer {
    pub node: Arc<NetworkNode>,
}

#[tonic::async_trait]
impl FileSystemService for FileSystemServer {
    async fn start(&self, _: Request<StartRequest>) -> Result<Response<StartResponse>, Status> {
        self.node.start();
        let response = StartResponse {};
        Ok(Response::new(response))
    }

    async fn list_files(
        &self,
        _: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let files = self.node.list_files();
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
        let temp_path = self.node.get_file(hash.into()).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(4);

        tokio::spawn(async move {
            let mut file = File::open(&temp_path).await?;
            let mut buffer = vec![0u8; 1024 * 1024];
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
        let peers = self.node.get_peers().await;
        Ok(Response::new(ListPeersResponse { peers }))
    }

    async fn add_peer(
        &self,
        request: Request<AddPeerRequest>,
    ) -> Result<Response<AddPeerResponse>, Status> {
        let peer_address = request.into_inner().peer_address;
        let success = self.node.add_peer(&peer_address.parse());
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

    async fn get_chunk(
        &self,
        request: Request<GetChunkRequest>,
    ) -> Result<Response<GetChunkResponse>, Status> {
        let chunk_hash = request.into_inner().chunk_hash;
        let fs = self.node.inner.fs.lock().await;
        if let Some(chunk) = fs
            .get_chunk(
                &chunk_hash
                    .try_into()
                    .map_err(|_| Status::invalid_argument("Invalid chunk hash"))?,
            )
            .await
        {
            Ok(Response::new(GetChunkResponse { chunk_data: chunk }))
        } else {
            Err(Status::not_found("Chunk not found"))
        }
    }

    async fn get_file(
        &self,
        request: Request<GetFileRequest>,
    ) -> Result<Response<GetFileResponse>, Status> {
        let file_hash = request.into_inner().file_hash;
        let fs = self.node.inner.fs.lock().await;
        if let Some(metadata) = fs.get_file_metadata(
            &file_hash
                .try_into()
                .map_err(|_| Status::invalid_argument("Invalid file hash"))?,
        ) {
            Ok(Response::new(GetFileResponse {
                metadata: Some(metadata.into()),
            }))
        } else {
            Err(Status::not_found("File not found"))
        }
    }

    async fn sync(&self, _: Request<SyncRequest>) -> Result<Response<SyncResponse>, Status> {
        let fs = self.node.inner.fs.lock().await;
        let files = fs
            .list_files()
            .into_iter()
            .map(|(hash, name)| FileInfo {
                name,
                hash: hash.to_vec(),
                total_size: 0,        // Adjust based on your FileSystem implementation
                chunk_hashes: vec![], // Populate if metadata is available
            })
            .collect();
        Ok(Response::new(SyncResponse { files }))
    }

    async fn notify_new_file(
        &self,
        request: Request<NewFileRequest>,
    ) -> Result<Response<NewFileResponse>, Status> {
        let file_hash = request.into_inner().file_hash;
        // Optionally trigger a fetch or just note the file's existence
        let mut fs = self.node.inner.fs.lock().await;
        // Placeholder: Add logic to handle new file notification if needed
        Ok(Response::new(NewFileResponse {}))
    }

    async fn get_uptime(
        &self,
        _: Request<GetUptimeRequest>,
    ) -> Result<Response<GetUptimeResponse>, Status> {
        let uptime_seconds = self.node.get_uptime();
        Ok(Response::new(GetUptimeResponse { uptime_seconds }))
    }
}
