use crate::network::NetworkNode;
use native_dialog::FileDialog;
use std::error::Error;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

pub async fn run() -> Result<(), Box<dyn Error>> {
    // Get port from user
    print!("Enter port number to listen on: ");
    io::stdout().flush()?;
    let port: u16 = read_line().parse()?;

    // Initialize node
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
    let mut node = NetworkNode::new(addr);

    // Optional: Connect to a peer
    print!("Enter peer address to connect to (leave empty for none): ");
    io::stdout().flush()?;
    let peer_port = read_line();
    if !peer_port.is_empty() {
        let peer_addr: SocketAddr = format!("127.0.0.1:{}", peer_port).parse()?;
        node.add_peer(peer_addr).await;
        println!("Connected to peer!");
    }

    println!("Node started successfully! Listening on {}", addr);

    // Start network listener
    let node_clone = node.clone();
    tokio::spawn(async move {
        if let Err(e) = node_clone.start().await {
            eprintln!("Network error: {}", e);
        }
    });

    // Periodically ping peers
    let node_clone2 = node.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            node_clone2.ping_self_nodes().await;
        }
    });

    // Main input loop
    loop {
        display_menu().await;
        match read_line().as_str() {
            "1" => handle_upload(&mut node).await?,
            "2" => handle_download(&node).await?,
            "3" => handle_list(&node).await?,
            "4" => list_peers(&node).await,
            "5" => {
                break;
            }
            _ => println!("Invalid choice!"),
        }
    }

    Ok(())
}

async fn handle_upload(node: &mut NetworkNode) -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=== File Upload ===");

    // Open a file dialog to select the file
    let path = FileDialog::new()
        .add_filter("Text files", &["txt"])
        .add_filter("Image files", &["jpg", "png", "gif"])
        .show_open_single_file()?;

    if let Some(path) = path {
        // Proceed with uploading the file
        if !path.exists() {
            println!("{}", "Error: File does not exist!");
            return Ok(());
        }

        let data = tokio::fs::read(&path).await?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid filename")?;

        let fs = node.get_filesystem().await;
        let file_hash = fs.lock().await.add_file(filename, &data).await;

        println!("\n{}", "=== Upload Successful ===");
        println!("File: {}", filename);
        println!("Size: {} bytes", data.len().to_string());
        println!("Hash: {}", hex::encode(file_hash));

        // Broadcast to peers that a new file is available
        node.broadcast_new_file(file_hash).await?;
    }

    Ok(())
}
async fn handle_download(node: &NetworkNode) -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=== File Download ===");
    print!("Enter file hash: ");
    io::stdout().flush()?;
    let hash_str = read_line();

    let hash_bytes = hex::decode(hash_str)?;
    let mut file_hash = [0u8; 32];
    file_hash.copy_from_slice(&hash_bytes);

    print!("Enter output path: ");
    io::stdout().flush()?;
    let output = PathBuf::from(read_line());

    println!("\n{}", "Downloading file...");
    let (filename, data) = node.get_file(file_hash).await?;
    tokio::fs::write(&output, data).await?;

    println!("\n{}", "=== Download Successful ===");
    println!("Original filename: {}", filename);
    println!("Saved to: {}", output.display().to_string());

    Ok(())
}

async fn handle_list(node: &NetworkNode) -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=== Available Files ===");
    let fs = node.get_filesystem().await;
    let files = fs.lock().await.list_files();

    if files.is_empty() {
        println!("{}", "No files in the network");
        return Ok(());
    }

    for (hash, name) in files {
        println!("📄 {}", name);
        println!("   Hash: {}", hex::encode(hash));
    }

    Ok(())
}

async fn list_peers(node: &NetworkNode) {
    println!("\n{}", "===listing peers===");
    let peers = node.get_peers().await;
    let peers = peers.lock().await;

    if !peers.is_empty() {
        for peer in &*peers {
            println!("\n{:?}", peer);
        }
    }
}

fn ping_peers(node: &NetworkNode) {}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    input.trim().to_string()
}

async fn display_menu() {
    println!("\n{}", "=== Main Menu ===");
    println!("1. Upload File");
    println!("2. Download File");
    println!("3. List Files");
    println!("4. List Peers");
    println!("5. Exit");
    print!("Enter your choice: ");
    io::stdout().flush().unwrap();
}
