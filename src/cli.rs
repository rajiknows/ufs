use crate::storage_proto::{
    peer_service_client::PeerServiceClient, GetChunkRequest, GetFileMetadataRequest,
    StoreChunkRequest, StoreFileMetadataRequest,
};
use crate::CliCommands;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use tonic::transport::Channel;
use tonic::Request;

pub async fn handle_cli_command(
    node_addr: String,
    command: CliCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PeerServiceClient::connect(node_addr).await?;

    match command {
        CliCommands::Upload { path } => {
            let path = Path::new(&path);
            let mut file = File::open(path)?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;

            // Example: fixed chunk size (64 KiB)
            let chunk_size = 64 * 1024;
            let mut chunk_hashes = Vec::new();

            for chunk in data.chunks(chunk_size) {
                let hash = sha2::Sha256::digest(chunk).as_slice().to_vec();
                client
                    .store_chunk(Request::new(StoreChunkRequest {
                        chunk_hash: hash.clone(),
                        chunk_data: chunk.to_vec(),
                    }))
                    .await?;
                chunk_hashes.push(hash);
            }

            let metadata = crate::storage_proto::FileInfo {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                size: data.len() as u64,
                chunk_hashes: chunk_hashes.clone(),
            };

            let file_hash = sha2::Sha256::digest(&bincode::serialize(&metadata)?)
                .as_slice()
                .to_vec();
            client
                .store_file_metadata(Request::new(StoreFileMetadataRequest {
                    file_hash,
                    metadata: bincode::serialize(&metadata)?,
                }))
                .await?;

            println!("File uploaded successfully.");
        }

        CliCommands::Download { hash, output } => {
            let file_hash_bytes = hex::decode(hash)?;
            let meta_resp = client
                .get_file_metadata(Request::new(GetFileMetadataRequest {
                    file_hash: file_hash_bytes.clone(),
                }))
                .await?
                .into_inner();

            let metadata: crate::storage_proto::FileInfo =
                bincode::deserialize(&meta_resp.metadata)?;

            let mut assembled = Vec::new();
            for chash in metadata.chunk_hashes {
                let chunk_resp = client
                    .get_chunk(Request::new(GetChunkRequest {
                        chunk_hash: chash.clone(),
                    }))
                    .await?
                    .into_inner();
                assembled.extend_from_slice(&chunk_resp.chunk_data);
            }

            std::fs::write(&output, assembled)?;
            println!(
                "File `{}` downloaded successfully to {:?}",
                metadata.name, output
            );
        }

        CliCommands::ListFiles => {
            let response = client
                .list_files(Request::new(crate::storage_proto::ListFilesRequest {}))
                .await?
                .into_inner();
            println!("Known files:");
            for file in response.files {
                println!("- Name: {}, Size: {}", file.name, file.size);
            }
        }

        CliCommands::ListPeers => {
            let response = client
                .list_peers(Request::new(crate::storage_proto::ListPeersRequest {}))
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
