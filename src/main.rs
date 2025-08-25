use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
mod dht;
mod utils;

mod node;
mod server;
mod storage;

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
    Server(ServerArgs),
    Cli(CliArgs),
}

#[derive(Parser, Debug)]
struct ServerArgs {
    #[arg(long, default_value_t = 42069)]
    port: u16,
    #[arg(long)]
    bootstrap_peer: Option<String>,
}

#[derive(Parser, Debug)]
struct CliArgs {
    #[arg(long, default_value = "http://127.0.0.1:42069")]
    node_addr: String,
    #[command(subcommand)]
    command: CliCommands,
}

#[derive(Subcommand, Debug)]
enum CliCommands {
    Upload {
        #[arg(long)]
        path: PathBuf,
    },
    Download {
        #[arg(long)]
        hash: String,
        #[arg(long)]
        output: PathBuf,
    },
    ListFiles,
    ListPeers,
    ShowChunks,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    match args.command {
        Commands::Server(server_args) => {
            server::start_server(server_args.port, server_args.bootstrap_peer).await?;
        }
        Commands::Cli(cli_args) => {
            cli::handle_cli_command(cli_args.node_addr, cli_args.command).await?;
        }
    }

    Ok(())
}
