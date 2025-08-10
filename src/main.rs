use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
mod gossip;
mod node;
mod server;
mod storage;

// Build gRPC code from proto file
pub mod storage_proto {
    tonic::include_proto!("storage");
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the P2P node server
    Server(ServerArgs),
    /// Interact with a P2P node via the CLI
    Cli(CliArgs),
}

#[derive(Parser, Debug)]
struct ServerArgs {
    /// Port to listen on for gRPC connections.
    #[arg(long, default_value_t = 50051)]
    port: u16,
    /// An optional peer to bootstrap this node's peer list from.
    #[arg(long)]
    bootstrap_peer: Option<String>,
    /// Path to the database directory.
    #[arg(long, default_value = "./db")]
    db_path: PathBuf,
}

#[derive(Parser, Debug)]
struct CliArgs {
    /// The address of the node to connect to.
    #[arg(
        long,
        default_value = "[http://127.0.0.1:50051](http://127.0.0.1:50051)"
    )]
    node_addr: String,
    #[command(subcommand)]
    command: CliCommands,
}

#[derive(Subcommand, Debug)]
enum CliCommands {
    /// Upload a file to the network.
    Upload {
        #[arg(long)]
        path: PathBuf,
    },
    /// Download a file from the network using its hash.
    Download {
        #[arg(long)]
        hash: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// List all files known to the connected node.
    ListFiles,
    /// List all peers known to the connected node.
    ListPeers,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    match args.command {
        Commands::Server(server_args) => {
            server::start_server(
                server_args.port,
                server_args.bootstrap_peer,
                server_args.db_path,
            )
            .await?;
        }
        Commands::Cli(cli_args) => {
            cli::handle_cli_command(cli_args.node_addr, cli_args.command).await?;
        }
    }

    Ok(())
}
