use crate::gossip::Gossip;
use crate::storage::Storage;
use crate::storage_proto::peer_service_client::PeerServiceClient;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Node {
    storage: Arc<Storage>,
    peers: Arc<Mutex<Vec<String>>>,
    gossip: Arc<Gossip>,
    address: String,
}

impl Node {
    pub fn new(db_path: &str, address: String) -> Result<Self, Box<dyn std::error::Error>> {
        let storage = Arc::new(Storage::new(db_path)?);
        let peers = Arc::new(Mutex::new(Vec::new()));
        let gossip = Arc::new(Gossip::new(peers.clone(), storage.clone()));

        Ok(Node {
            storage,
            peers,
            gossip,
            address,
        })
    }

    pub async fn start(
        &self,
        bootstrap_peer: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(addr) = bootstrap_peer {
            self.bootstrap(addr).await?;
        }
        let gossip_clone = self.gossip.clone();
        tokio::spawn(async move {
            gossip_clone.start().await;
        });
        Ok(())
    }

    async fn bootstrap(&self, addr: String) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Bootstrapping with peer at {}", addr);
        let mut client = PeerServiceClient::connect(addr).await?;
        let response = client
            .share_peers(tonic::Request::new(crate::storage_proto::PeerRequest {
                known_peers: self.get_peers().await,
                from_address: self.address.clone(),
            }))
            .await?
            .into_inner();

        self.add_peers(response.known_peers).await;
        Ok(())
    }

    pub async fn get_peers(&self) -> Vec<String> {
        self.peers.lock().await.clone()
    }

    pub async fn add_peers(&self, peers: Vec<String>) {
        for peer in peers {
            self.add_peer(peer).await;
        }
    }

    pub async fn add_peer(&self, peer: String) {
        let mut peers = self.peers.lock().await;
        if !peers.contains(&peer) {
            peers.push(peer);
        }
    }

    pub async fn store_chunk(&self, hash: &[u8], data: &[u8]) -> Result<(), sled::Error> {
        self.storage.store_chunk(hash, data)
    }

    pub async fn get_chunk(&self, hash: &[u8]) -> Result<Option<Vec<u8>>, sled::Error> {
        self.storage.get_chunk(hash)
    }

    pub async fn get_metadata(
        &self,
        hash: &[u8],
    ) -> Result<Option<crate::storage::FileInfo>, Box<dyn std::error::Error>> {
        self.storage.get_metadata(hash)
    }

    pub async fn get_all_metadata(
        &self,
    ) -> Result<Vec<crate::storage::FileInfo>, Box<dyn std::error::Error>> {
        self.storage.get_all_metadata()
    }

    pub async fn handle_gossip(&self, hashes: Vec<Vec<u8>>) {
        log::info!("Handling gossip with {} hashes", hashes.len());
        let known_hashes = self.storage.get_all_chunk_hashes().unwrap_or_default();

        for hash in hashes {
            if !known_hashes.contains(&hash) {
                log::info!(
                    "Requesting metadata for unknown hash {}",
                    hex::encode(&hash)
                );
                let peers = self.get_peers().await;
                if peers.is_empty() {
                    log::warn!("No peers to request metadata from.");
                    continue;
                }
                let random_peer = peers[rand::random::<usize>() % peers.len()].clone();
                match self.request_metadata(&random_peer, &hash).await {
                    Ok(_) => log::info!("Successfully requested metadata"),
                    Err(e) => log::warn!("Failed to request metadata: {e}"),
                }
            }
        }
    }

    async fn request_metadata(
        &self,
        peer_addr: &str,
        hash: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = PeerServiceClient::connect(peer_addr.to_string()).await?;
        let request = tonic::Request::new(crate::storage_proto::GetFileMetadataRequest {
            file_hash: hash.to_vec(),
        });
        let response = client.get_file_metadata(request).await?.into_inner();
        let metadata: crate::storage::FileInfo = bincode::deserialize(&response.metadata)?;
        self.storage.store_metadata(hash, &metadata)?;
        Ok(())
    }
}
