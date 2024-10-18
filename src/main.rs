pub mod fs;
pub mod network;
pub mod routing;
pub mod storage;
use std::io::{self, Write};
use std::net::SocketAddr;

use clap::Parser;
use network::Network;

#[derive(Parser)]
#[clap(
    name = "IPFS-like CLI",
    version = "1.0",
    author = "Your Name",
    about = "A simple IPFS-like peer-to-peer network"
)]
struct Cli {
    #[clap(
        short,
        long,
        value_name = "HOST",
        help = "Sets the host address for this client"
    )]
    host: String,

    #[clap(
        short,
        long,
        value_name = "PORT",
        help = "Sets the port for this client"
    )]
    port: u16,

    #[clap(
        short,
        long,
        value_name = "CONNECT",
        help = "Specifies a node to connect to (host:port)"
    )]
    connect: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let client_addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let mut bootstrap_nodes = vec![];
    if let Some(connect_addr) = cli.connect {
        bootstrap_nodes.push(connect_addr.parse()?);
    }

    let network = Network::new(client_addr, bootstrap_nodes).await?;

    println!("Starting network client...");
    // Start the network in the background
    let network_clone = network.clone();
    tokio::spawn(async move {
        if let Err(e) = network_clone.run().await {
            eprintln!("Network error: {}", e);
        }
    });

    println!("Interactive IPFS-like CLI");
    println!(
        "Available commands: upload <file_path>, get <file_hash>, list, discover, nodes, quit"
    );

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "upload" => {
                if parts.len() != 2 {
                    println!("Usage: upload <file_path>");
                } else {
                    let file_path = parts[1];
                    let data = std::fs::read(file_path)?;
                    let hash = network.upload_file(file_path, &data).await?;
                    println!("File uploaded with hash: {}", hex_encode(&hash));
                }
            }
            "get" => {
                if parts.len() != 2 {
                    println!("Usage: get <file_hash>");
                } else {
                    let hash_str = parts[1];
                    match hex_decode(hash_str) {
                        Ok(hash) => match network.get_file(&hash).await? {
                            Some((name, data)) => {
                                println!("Retrieved file: {} ({} bytes)", name, data.len());
                                std::fs::write(&name, data)?;
                                println!("File saved as: {}", name);
                            }
                            None => println!("File not found"),
                        },
                        Err(_) => {
                            println!("Invalid hash format. Expected 64 hexadecimal characters.")
                        }
                    }
                }
            }
            "list" => {
                let files = network.list_files().await?;
                println!("Files in the network:");
                for file in files {
                    println!("- {}", file);
                }
            }
            "discover" => {
                network.discover_nodes().await?;
                println!("Node discovery completed");
            }
            "nodes" => {
                let nodes = network.list_nodes().await?;
                println!("Known nodes:");
                for node in nodes {
                    println!("- {}", node.addr);
                }
            }
            "quit" => {
                println!("Exiting...");
                break;
            }
            _ => {
                println!("Unknown command. Available commands: upload <file_path>, get <file_hash>, list, discover, nodes, quit");
            }
        }
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    if s.len() != 64 {
        return Err("Invalid hash length".into());
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        bytes[i] = u8::from_str_radix(std::str::from_utf8(chunk)?, 16)?;
    }
    Ok(bytes)
}
