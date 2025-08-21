use crate::dht::{Peer, RoutingTable, K_VALUE};
use crate::storage::{FileInfo, Storage};
use crate::storage_proto::peer_service_client::PeerServiceClient;
use crate::storage_proto::{FindNodeRequest, FindValueRequest, PeerMessage, PingRequest};
use crate::utils::hash;
use futures::future::join_all;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::Request;

#[derive(Clone)]
pub struct Node {
    pub id: [u8; 32],
    pub address: String,
    pub storage: Arc<Storage>,
    pub routing_table: Arc<Mutex<RoutingTable>>,
}

impl Node {
    pub fn new(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let id = hash(address.as_bytes()).try_into().unwrap();
        let storage = Arc::new(Storage::new());
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(id)));

        Ok(Node {
            id,
            address: address.to_string(),
            storage,
            routing_table,
        })
    }

    pub async fn start(
        &self,
        bootstrap_peer: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(addr) = bootstrap_peer {
            self.bootstrap(&addr).await?;
        }
        Ok(())
    }

    async fn bootstrap(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Bootstrapping with peer at {}", addr);
        let mut client = PeerServiceClient::connect(addr.to_string()).await?;

        let response = client
            .ping(Request::new(PingRequest {
                peer: Some(PeerMessage {
                    node_id: self.id.to_vec(),
                    address: self.address.clone(),
                }),
            }))
            .await?;

        let bootstrap_node_id: [u8; 32] = response.into_inner().node_id.try_into().unwrap();

        let bootstrap_peer = Peer {
            node_id: bootstrap_node_id,
            address: addr.to_string(),
        };

        self.routing_table.lock().await.add_peer(bootstrap_peer);

        // Perform a FIND_NODE on ourself to discover the network
        self.perform_find_node(&self.id).await?;

        Ok(())
    }

    pub async fn perform_find_node(
        &self,
        target_id: &[u8; 32],
    ) -> Result<Vec<Peer>, Box<dyn std::error::Error>> {
        let mut closest_peers = self
            .routing_table
            .lock()
            .await
            .find_closest_peers(target_id);
        let mut queried_peers = HashSet::new();
        let mut results = Vec::new();

        loop {
            let mut futures = Vec::new();
            let peers_to_query: Vec<Peer> = closest_peers
                .iter()
                .filter(|p| !queried_peers.contains(&p.node_id))
                .take(K_VALUE)
                .cloned()
                .collect();

            if peers_to_query.is_empty() {
                break;
            }

            for peer in peers_to_query {
                queried_peers.insert(peer.node_id);
                let future = async move {
                    log::info!("Querying peer {:?} for target", peer.address);
                    let mut client = PeerServiceClient::connect(peer.address).await?;
                    let request = Request::new(FindNodeRequest {
                        target_id: target_id.to_vec(),
                    });
                    let response = client.find_node(request).await?;
                    let peers: Vec<Peer> = response
                        .into_inner()
                        .peers
                        .into_iter()
                        .map(|p| Peer {
                            node_id: p.node_id.try_into().unwrap(),
                            address: p.address,
                        })
                        .collect();
                    Ok::<_, Box<dyn std::error::Error>>(peers)
                };
                futures.push(future);
            }

            let responses = join_all(futures).await;
            let mut found_new_peers = false;

            for res in responses {
                if let Ok(peers) = res {
                    for peer in peers {
                        if !closest_peers.iter().any(|p| p.node_id == peer.node_id) {
                            closest_peers.push(peer);
                            found_new_peers = true;
                        }
                    }
                }
            }

            closest_peers.sort_by_key(|p| crate::dht::xor_distance(&p.node_id, target_id));
            results = closest_peers.clone();

            if !found_new_peers {
                break;
            }
        }

        Ok(results.into_iter().take(K_VALUE).collect())
    }

    pub async fn perform_find_value(
        &self,
        key: &[u8; 32],
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut closest_peers = self.routing_table.lock().await.find_closest_peers(key);
        let mut queried_peers = HashSet::new();

        loop {
            let mut futures = Vec::new();
            let peers_to_query: Vec<Peer> = closest_peers
                .iter()
                .filter(|p| !queried_peers.contains(&p.node_id))
                .take(K_VALUE)
                .cloned()
                .collect();

            if peers_to_query.is_empty() {
                break;
            }

            for peer in peers_to_query {
                queried_peers.insert(peer.node_id);
                let future = async move {
                    let mut client = PeerServiceClient::connect(peer.address).await?;
                    let request = Request::new(FindValueRequest { key: key.to_vec() });
                    let response = client.find_value(request).await?;
                    Ok::<_, Box<dyn std::error::Error>>(response.into_inner())
                };
                futures.push(future);
            }

            let responses = join_all(futures).await;
            let mut found_new_peers = false;

            for res in responses {
                if let Ok(response) = res {
                    if let Some(result) = response.result {
                        match result {
                            crate::storage_proto::find_value_response::Result::Value(v) => {
                                return Ok(Some(v));
                            }
                            crate::storage_proto::find_value_response::Result::ClosestPeers(p) => {
                                for peer_msg in p.peers {
                                    let peer = Peer {
                                        node_id: peer_msg.node_id.try_into().unwrap(),
                                        address: peer_msg.address,
                                    };
                                    if !closest_peers.iter().any(|p| p.node_id == peer.node_id) {
                                        closest_peers.push(peer);
                                        found_new_peers = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            closest_peers.sort_by_key(|p| crate::dht::xor_distance(&p.node_id, key));

            if !found_new_peers {
                break;
            }
        }

        Ok(None)
    }

    pub fn store_chunk(&self, hash: &[u8], data: &[u8]) {
        self.storage.store_chunk(hash, data)
    }

    pub fn get_chunk(&self, hash: &[u8]) -> Option<Vec<u8>> {
        self.storage.get_chunk(hash)
    }

    pub fn store_metadata(&self, hash: &[u8], metadata: &FileInfo) {
        self.storage.store_metadata(hash, metadata)
    }

    pub fn get_metadata(&self, hash: &[u8]) -> Option<FileInfo> {
        self.storage.get_metadata(hash)
    }

    pub fn get_all_metadata(&self) -> Vec<FileInfo> {
        self.storage.get_all_metadata()
    }
}
