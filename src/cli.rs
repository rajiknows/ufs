use crate::storage_proto::peer_service_client::PeerServiceClient;
use crate::CliCommands;
use tonic::transport::Channel;

/// Handles all client-side commands.
pub async fn handle_cli_command(
    node_addr: String,
    command: CliCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PeerServiceClient::connect(node_addr).await?;

    match command {
        CliCommands::Upload { path } => {
            // TODO: Implement file upload logic.
            // 1. Read the file from `path`.
            // 2. Chunk the file.
            // 3. Hash each chunk and the file metadata.
            // 4. Tell the connected node to store the file.
            //    This will involve new gRPC calls, e.g., `InitiateUpload`.
            println!("Uploading file from path: {:?}", path);
            println!("FEATURE NOT IMPLEMENTED YET");
        }
        CliCommands::Download { hash, output } => {
            // TODO: Implement file download logic.
            // 1. Ask the node for the file's metadata using the hash.
            // 2. From the metadata, get the list of chunk hashes.
            // 3. For each chunk hash, request the chunk data from the network.
            // 4. Reassemble the chunks and write to the `output` path.
            println!("Downloading file with hash `{}` to `{:?}`", hash, output);
            println!("FEATURE NOT IMPLEMENTED YET");
        }
        CliCommands::ListFiles => {
            let response = client.list_files(tonic::Request::new(crate::storage_proto::ListFilesRequest {})).await?.into_inner();
            println!("Known files:");
            for file in response.files {
                println!("- Name: {}, Size: {}", file.name, file.size);
            }
        }
        CliCommands::ListPeers => {
            let response = client.list_peers(tonic::Request::new(crate::storage_proto::ListPeersRequest {})).await?.into_inner();
            println!("Known peers:");
            for peer in response.peers {
                println!("- {}", peer);
            }
        }
    }

    Ok(())
}
