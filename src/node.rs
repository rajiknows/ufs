use crate::gossip::Gossip;
use crate::storage::Storage;
use crate::storage_proto::peer_service_client::PeerServiceClient;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

/// The central state and logic container for a peer.
#[derive(Clone)]
pub struct Node {
    /// The persistent key-value store for chunks and metadata.
    storage: Arc<Storage>,
    /// The list of known peer addresses (e.g., "[http://127.0.0.1:50051](http://127.0.0.1:50051)").
    peers: Arc<Mutex<Vec<String>>>,
    /// The gossip protocol handler.
    gossip: Arc<Gossip>,
}

impl Node {
    /// Creates a new Node.
    pub fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let storage = Arc::new(Storage::new(db_path)?);
        let peers = Arc::new(Mutex::new(Vec::new()));
        let gossip = Arc::new(Gossip::new(peers.clone()));

        Ok(Node {
            storage,
            peers,
            gossip,
        })
    }

    /// Starts the node's background tasks.
    pub async fn start(
        &self,
        bootstrap_peer: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If a bootstrap peer is provided, connect to it to get an initial peer list.
        if let Some(addr) = bootstrap_peer {
            self.bootstrap(addr).await?;
        }

        // Start the gossip background task.
        let gossip_clone = self.gossip.clone();
        tokio::spawn(async move {
            gossip_clone.start().await;
        });

        Ok(())
    }

    /// Connects to a bootstrap peer to populate the initial peer list.
    async fn bootstrap(&self, addr: String) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Bootstrapping with peer at {}", addr);
        // TODO:
        // 1. Create a gRPC client to the bootstrap peer.
        // 2. Call the `SharePeers` RPC.
        // 3. Add the returned peers to our own peer list.
        // self.add_peers(response.known_peers).await;
        Ok(())
    }

    /// Retrieves a list of all known peers.
    pub async fn get_peers(&self) -> Vec<String> {
        self.peers.lock().await.clone()
    }

    /// Stores a chunk in the local database.
    pub async fn store_chunk(&self, hash: &[u8], data: &[u8]) -> Result<(), rocksdb::Error> {
        self.storage.store_chunk(hash, data)
    }

    /// Retrieves a chunk from the local database.
    pub async fn get_chunk(&self, hash: &[u8]) -> Result<Option<Vec<u8>>, rocksdb::Error> {
        self.storage.get_chunk(hash)
    }
}
