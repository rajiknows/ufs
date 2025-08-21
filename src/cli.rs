use crate::storage_proto::peer_service_client::PeerServiceClient;
use crate::storage_proto::{
    FileInfo, GetChunkRequest, GetFileMetadataRequest, InitiateUploadRequest, UploadChunkRequest,
};
use crate::utils::hash;
use crate::CliCommands;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tonic::Request;

/// Handles all client-side commands.
pub async fn handle_cli_command(
    node_addr: String,
    command: CliCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PeerServiceClient::connect(node_addr).await?;

    match command {
        CliCommands::Upload { path } => {
            let file_hash = upload_file(&mut client, path).await?;
            println!("File uploaded successfully. Hash: {}", file_hash);
        }
        CliCommands::Download { hash, output } => {
            download_file(&mut client, &hash, output).await?;
            println!("File downloaded successfully.");
        }
        CliCommands::ListFiles => {
            let response = client
                .list_files(tonic::Request::new(
                    crate::storage_proto::ListFilesRequest {},
                ))
                .await?
                .into_inner();
            println!("Known files:");
            for file in response.files {
                println!("- Name: {}, Size: {}", file.name, file.size);
            }
        }
        CliCommands::ListPeers => {
            let response = client
                .list_peers(tonic::Request::new(
                    crate::storage_proto::ListPeersRequest {},
                ))
                .await?
                .into_inner();
            println!("Known peers:");
            for peer in response.peers {
                println!("- {}", peer);
            }
        }
    }

    Ok(())
}

async fn upload_file(
    client: &mut PeerServiceClient<tonic::transport::Channel>,
    path: PathBuf,
) -> Result<String, Box<dyn std::error::Error>> {
    let data = fs::read(&path)?;
    let chunk_size = 1024 * 256;
    let chunks: Vec<Vec<u8>> = data.chunks(chunk_size).map(|c| c.to_vec()).collect();
    let chunk_hashes: Vec<Vec<u8>> = chunks.iter().map(|c| hash(c)).collect();
    let metadata = FileInfo {
        name: path.file_name().unwrap().to_string_lossy().into(),
        size: data.len() as u64,
        chunk_hashes: chunk_hashes.clone(),
    };
    let file_hash = hash(&bincode::serialize(&metadata)?);

    client
        .initiate_upload(Request::new(InitiateUploadRequest {
            file_hash: file_hash.clone(),
            metadata: Some(metadata),
        }))
        .await?;

    for (chunk, hash) in chunks.iter().zip(chunk_hashes.iter()) {
        client
            .upload_chunk(Request::new(UploadChunkRequest {
                chunk_hash: hash.clone(),
                chunk_data: chunk.clone(),
            }))
            .await?;
    }
    Ok(hex::encode(file_hash))
}

async fn download_file(
    client: &mut PeerServiceClient<tonic::transport::Channel>,
    hash: &str,
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_hash = hex::decode(hash)?;

    let metadata_response = client
        .get_file_metadata(Request::new(GetFileMetadataRequest {
            file_hash: file_hash.clone(),
        }))
        .await?
        .into_inner();

    let metadata: FileInfo = bincode::deserialize(&metadata_response.metadata)?;

    let mut file = fs::File::create(output)?;

    for chunk_hash in metadata.chunk_hashes {
        let chunk_response = client
            .get_chunk(Request::new(GetChunkRequest { chunk_hash }))
            .await?
            .into_inner();
        file.write_all(&chunk_response.chunk_data)?;
    }

    Ok(())
}
