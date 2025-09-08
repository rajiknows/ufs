use crate::storage_proto::peer_service_client::PeerServiceClient;
use crate::storage_proto::{
    replicate_chunks_request, FileInfo, GetChunkRequest, GetFileMetadataRequest,
    InitiateUploadRequest, ReplicateChunksRequest, StoreRequest, UploadChunkRequest,
};
use crate::utils::hash;
use crate::{node, CliCommands};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tonic::{Request, Status};

pub async fn handle_cli_command(
    node_addr: String,
    command: CliCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        CliCommands::Upload { path } => {
            upload_file(&node_addr, path).await?;
        }
        CliCommands::Download { hash, output } => {
            download_file(&node_addr, &hash, output).await?;
        }
        CliCommands::ListFiles => {
            let mut client = PeerServiceClient::connect(node_addr).await?;
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
            let mut client = PeerServiceClient::connect(node_addr).await?;
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
        CliCommands::ShowChunks => {
            let mut client = PeerServiceClient::connect(node_addr).await?;
            let response = client
                .show_chunks(tonic::Request::new(
                    crate::storage_proto::ShowChunksRequest {},
                ))
                .await?
                .into_inner();
            println!("Chunks on local: ");
            for chunk in response.chunks {
                println!("- {}", hex::encode(chunk));
            }
        }
    }

    Ok(())
}

use futures::stream;

pub async fn upload_file(node_addr: &str, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PeerServiceClient::connect(node_addr.to_string()).await?;

    let data = fs::read(&path)?;
    let chunk_size = 1024 * 256;
    let chunks: Vec<Vec<u8>> = data.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let chunk_hashes: Vec<Vec<u8>> = chunks.iter().map(|c| hash(c)).collect();

    let metadata = FileInfo {
        name: path.file_name().unwrap().to_string_lossy().into(),
        size: data.len() as u64,
        chunk_hashes: chunk_hashes.clone(),
    };

    let file_hash_vec = hash(&bincode::serialize(&metadata)?);
    let file_hash: [u8; 32] = file_hash_vec.as_slice().try_into().unwrap();

    client
        .initiate_upload(Request::new(InitiateUploadRequest {
            file_hash: file_hash_vec.clone(),
            metadata: Some(metadata.clone()),
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

    println!("File uploaded locally. Hash: {}", hex::encode(file_hash));

    let mut find_client = PeerServiceClient::connect(node_addr.to_string()).await?;
    let find_node_response = find_client
        .find_node(Request::new(crate::storage_proto::FindNodeRequest {
            target_id: file_hash.to_vec(),
        }))
        .await?;

    let closest_peers = find_node_response.into_inner().peers;
    let mut handles = Vec::new();

    for peer in closest_peers {
        println!("Announcing file to peer at {}", peer.address);

        let mut store_client = PeerServiceClient::connect(peer.address.clone()).await?;
        store_client
            .store(Request::new(StoreRequest {
                key: file_hash.to_vec(),
                value: node_addr.to_string(),
            }))
            .await?;

        let metadata = metadata.clone();
        let chunks = chunks.clone();
        let file_hash_vec = file_hash_vec.clone();
        let peer_addr = peer.address.clone();
        let node_addr = node_addr.to_owned();

        let handle = tokio::spawn(async move {
            log::info!("started the spawn");

            let mut client = match PeerServiceClient::connect(peer_addr.clone()).await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("failed to connect to {}: {:?}", peer_addr, e);
                    return;
                }
            };
            log::info!("peer found and connected");

            let mut requests = Vec::new();
            requests.push(ReplicateChunksRequest {
                node_id: node_addr.as_bytes().to_vec(),
                file_hash: file_hash_vec.clone(),
                payload: Some(replicate_chunks_request::Payload::Metadata(metadata)),
            });
            for chunk in chunks {
                requests.push(ReplicateChunksRequest {
                    node_id: node_addr.as_bytes().to_vec(),
                    file_hash: file_hash_vec.clone(),
                    payload: Some(replicate_chunks_request::Payload::ChunkData(chunk)),
                });
            }

            let stream = stream::iter(requests);
            match client.replicate_chunks(Request::new(stream)).await {
                Ok(resp) => {
                    log::info!(
                        "replication response from {}: {:?}",
                        peer_addr,
                        resp.into_inner()
                    );
                }
                Err(e) => {
                    log::error!("replication to {} failed: {:?}", peer_addr, e);
                }
            }
        });

        handles.push(handle);
    }

    // wait for all replication tasks to finish
    for handle in handles {
        if let Err(e) = handle.await {
            log::error!("replication task panicked: {:?}", e);
        }
    }

    log::info!("finished");

    Ok(())
}
async fn download_file(
    node_addr: &str,
    hash_str: &str,
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_hash_vec = hex::decode(hash_str)?;
    let file_hash: [u8; 32] = file_hash_vec.as_slice().try_into().unwrap();

    // find the value (provider address) for the file hash
    let mut client = PeerServiceClient::connect(node_addr.to_string()).await?;
    let find_value_response = client
        .find_value(Request::new(crate::storage_proto::FindValueRequest {
            key: file_hash.to_vec(),
        }))
        .await?;

    let result = find_value_response.into_inner().result;

    if let Some(crate::storage_proto::find_value_response::Result::Value(provider_addr)) = result {
        println!("File provider found at: {}", provider_addr);

        // now connect to the provider and download the file
        let mut provider_client = PeerServiceClient::connect(provider_addr).await?;

        let metadata_response = provider_client
            .get_file_metadata(Request::new(GetFileMetadataRequest {
                file_hash: file_hash.to_vec(),
            }))
            .await?
            .into_inner();

        let metadata: FileInfo = bincode::deserialize(&metadata_response.metadata)?;

        let mut file = fs::File::create(output)?;

        for chunk_hash in metadata.chunk_hashes {
            let chunk_response = provider_client
                .get_chunk(Request::new(GetChunkRequest { chunk_hash }))
                .await?
                .into_inner();
            file.write_all(&chunk_response.chunk_data)?;
        }
        println!("File downloaded successfully.");
    } else {
        println!("File not found on the network.");
    }

    Ok(())
}
