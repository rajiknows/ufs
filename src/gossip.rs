use crate::storage::Storage;
use crate::storage_proto::peer_service_client::PeerServiceClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct Gossip {
    peers: Arc<Mutex<Vec<String>>>,
    storage: Arc<Storage>,
}

impl Gossip {
    pub fn new(peers: Arc<Mutex<Vec<String>>>, storage: Arc<Storage>) -> Self {
        Gossip { peers, storage }
    }

    pub async fn start(&self) {
        log::info!("Gossip protocol started.");
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let peers = self.peers.lock().await;
            if peers.is_empty() {
                log::warn!("No peers to gossip with.");
                continue;
            }

            log::info!("Executing gossip tick...");
            let random_peer = peers[rand::random::<u32>() as usize % peers.len()].clone();
            self.gossip_with_peer(&random_peer).await;
        }
    }

    async fn gossip_with_peer(&self, peer_addr: &str) {
        log::info!("Gossiping with peer at {}", peer_addr);
        match self.gossip_with_peer_fallible(peer_addr).await {
            Ok(_) => log::info!("Gossip with {} successful", peer_addr),
            Err(e) => log::warn!("Gossip with {} failed: {}", peer_addr, e),
        }
    }

    async fn gossip_with_peer_fallible(
        &self,
        peer_addr: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = PeerServiceClient::connect(peer_addr.to_string()).await?;
        let request = tonic::Request::new(crate::storage_proto::GossipMessage {
            file_hashes: self.storage.get_all_chunk_hashes()?,
        });
        client.gossip(request).await?;
        Ok(())
    }
}
