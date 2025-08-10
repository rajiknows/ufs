use crate::main::CliCommands;
use crate::storage_proto::peer_service_client::PeerServiceClient;
use tonic::transport::Channel;

/// Handles all client-side commands.
pub async fn handle_cli_command(
    node_addr: String,
    command: CliCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Connecting to node at {}", node_addr);
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
            // TODO: Add a gRPC endpoint to list files known by a node.
            println!("Listing files...");
            println!("FEATURE NOT IMPLEMENTED YET");
        }
        CliCommands::ListPeers => {
            // TODO: Add a gRPC endpoint to list peers known by a node.
            println!("Listing peers...");
            println!("FEATURE NOT IMPLEMENTED YET");
        }
    }

    Ok(())
}
