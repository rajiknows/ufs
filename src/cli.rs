use crate::storage_proto::peer_service_client::PeerServiceClient;
use crate::storage_proto::{
    FileInfo, GetChunkRequest, GetFileMetadataRequest, InitiateUploadRequest, StoreRequest,
    UploadChunkRequest,
};
use crate::utils::hash;
use crate::CliCommands;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tonic::Request;

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
    }

    Ok(())
}

async fn upload_file(node_addr: &str, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PeerServiceClient::connect(node_addr.to_string()).await?;

    let data = fs::read(&path)?;
    // 256kb chunks
    let chunk_size = 1024 * 256;
    let chunks: Vec<Vec<u8>> = data.chunks(chunk_size).map(|c| c.to_vec()).collect();
    // hash the chunks
    let chunk_hashes: Vec<Vec<u8>> = chunks.iter().map(|c| hash(c)).collect();
    // store the metadata
    let metadata = FileInfo {
        name: path.file_name().unwrap().to_string_lossy().into(),
        size: data.len() as u64,
        chunk_hashes: chunk_hashes.clone(),
    };
    // we hash the entire metadata and store it as file hash
    let file_hash_vec = hash(&bincode::serialize(&metadata)?);
    let file_hash: [u8; 32] = file_hash_vec.as_slice().try_into().unwrap();

    // Store the file locally
    client
        .initiate_upload(Request::new(InitiateUploadRequest {
            file_hash: file_hash_vec,
            metadata: Some(metadata),
        }))
        .await?;

    // store the chunks locally
    for (chunk, hash) in chunks.iter().zip(chunk_hashes.iter()) {
        client
            .upload_chunk(Request::new(UploadChunkRequest {
                chunk_hash: hash.clone(),
                chunk_data: chunk.clone(),
            }))
            .await?;
    }

    println!("File uploaded locally. Hash: {}", hex::encode(file_hash));

    // Find k-closest nodes to the file hash
    let mut find_client = PeerServiceClient::connect(node_addr.to_string()).await?;
    let find_node_response = find_client
        .find_node(Request::new(crate::storage_proto::FindNodeRequest {
            target_id: file_hash.to_vec(),
        }))
        .await?;

    let closest_peers = find_node_response.into_inner().peers;
    println!(
        "Found {} closest peers to announce the file to.",
        closest_peers.len()
    );

    // Send a Store Request to each of the k-closest nodes
    // telling them that we have the file
    for peer in closest_peers {
        println!("Announcing file to peer at {}", peer.address);
        let mut store_client = PeerServiceClient::connect(peer.address).await?;
        store_client
            .store(Request::new(StoreRequest {
                key: file_hash.to_vec(),
                value: node_addr.to_string(),
            }))
            .await?;
    }

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
