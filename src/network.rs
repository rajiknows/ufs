use crate::fs::FileSystem;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

type NodeId = [u8; 32];

#[derive(Deserialize, Serialize)]
enum Message {
    UploadFile { name: String, data: Vec<u8> },
    GetFile { hash: [u8; 32] },
    FileData { name: String, data: Vec<u8> },
    NotFound,
    DiscoverNodes,
    NodeList { nodes: Vec<Peer> },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Peer {
    pub id: NodeId,
    pub addr: SocketAddr,
}

//impl PartialEq for Peer {
//    fn eq(&self, other: &Self) -> bool {
//        self.id == other.id
//    }
//}

#[derive(Clone)]
pub struct Network {
    client_peer: Peer,
    nodes: Arc<Mutex<Vec<Peer>>>,
    fs: Arc<Mutex<FileSystem>>,
}

impl Network {
    pub async fn new(
        client_addr: SocketAddr,
        bootstrap_nodes: Vec<SocketAddr>,
    ) -> Result<Self, Box<dyn Error>> {
        let nodes = bootstrap_nodes
            .into_iter()
            .map(|addr| Peer {
                id: hash_socket_addr(&addr),
                addr,
            })
            .collect();

        Ok(Network {
            client_peer: Peer {
                id: hash_socket_addr(&client_addr),
                addr: client_addr,
            },
            nodes: Arc::new(Mutex::new(nodes)),
            fs: Arc::new(Mutex::new(FileSystem::new())),
        })
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(self.client_peer.addr).await?;
        println!("Listening on: {}", self.client_peer.addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let fs = self.fs.clone();
            let nodes = self.nodes.clone();
            tokio::spawn(async move {
                Self::handle_connection(socket, fs, nodes).await;
            });
        }
    }

    async fn handle_connection(
        mut socket: TcpStream,
        fs: Arc<Mutex<FileSystem>>,
        nodes: Arc<Mutex<Vec<Peer>>>,
    ) {
        let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
        while let Ok(n) = socket.read(&mut buffer).await {
            if n == 0 {
                return;
            }
            if let Ok(message) = bincode::deserialize::<Message>(&buffer[..n]) {
                match message {
                    Message::UploadFile { name, data } => {
                        let mut fs = fs.lock().await;
                        let hash = fs.add_file(&name, &data);
                        let response = bincode::serialize(&hash).unwrap();
                        socket.write_all(&response).await.unwrap();
                    }
                    Message::GetFile { hash } => {
                        let fs = fs.lock().await;
                        let response = match fs.get_file(&hash) {
                            Some((name, data)) => {
                                bincode::serialize(&Message::FileData { name, data }).unwrap()
                            }
                            None => bincode::serialize(&Message::NotFound).unwrap(),
                        };
                        socket.write_all(&response).await.unwrap();
                    }
                    Message::DiscoverNodes => {
                        let nodes = nodes.lock().await;
                        let response = bincode::serialize(&Message::NodeList {
                            nodes: nodes.clone(),
                        })
                        .unwrap();
                        socket.write_all(&response).await.unwrap();
                    }
                    _ => { /* Handle other message types */ }
                }
            }
        }
    }

    pub async fn upload_file(&self, name: &str, data: &[u8]) -> Result<[u8; 32], Box<dyn Error>> {
        let mut fs = self.fs.lock().await;
        let hash = fs.add_file(name, data);
        Ok(hash)
    }

    pub async fn get_file(
        &self,
        hash: &[u8; 32],
    ) -> Result<Option<(String, Vec<u8>)>, Box<dyn Error>> {
        let fs = self.fs.lock().await;
        Ok(fs.get_file(hash))
    }

    pub async fn list_files(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let fs = self.fs.lock().await;
        Ok(fs.list_file())
    }

    pub async fn discover_nodes(&self) -> Result<(), Box<dyn Error>> {
        let mut new_nodes = Vec::new();

        for peer in self.nodes.lock().await.iter() {
            if let Ok(mut stream) = TcpStream::connect(peer.addr).await {
                let message = bincode::serialize(&Message::DiscoverNodes)?;
                stream.write_all(&message).await?;

                let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
                let n = stream.read(&mut buffer).await?;

                if let Ok(Message::NodeList { nodes }) = bincode::deserialize(&buffer[..n]) {
                    new_nodes.extend(nodes);
                }
            }
        }

        let mut nodes = self.nodes.lock().await;
        for node in new_nodes {
            if !nodes.contains(&node) {
                nodes.push(node);
            }
        }

        Ok(())
    }

    pub async fn list_nodes(&self) -> Result<Vec<Peer>, Box<dyn Error>> {
        Ok(self.nodes.lock().await.clone())
    }
}

fn hash_socket_addr(addr: &SocketAddr) -> NodeId {
    let mut hasher = Sha256::new();
    hasher.update(addr.to_string().as_bytes());
    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}
