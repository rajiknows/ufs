use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const K_VALUE: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Peer {
    pub node_id: [u8; 32],
    pub address: String,
}

pub struct RoutingTable {
    pub local_node_id: [u8; 32],
    pub buckets: [VecDeque<Peer>; 256],
}

impl RoutingTable {
    pub fn new(local_node_id: [u8; 32]) -> Self {
        Self {
            local_node_id,
            buckets: std::array::from_fn(|_| VecDeque::with_capacity(K_VALUE)),
        }
    }

    pub fn add_peer(&mut self, peer: Peer) {
        if self.local_node_id == peer.node_id {
            return;
        }

        let bucket_index = self.bucket_index(&peer.node_id);
        let bucket = &mut self.buckets[bucket_index];

        if let Some(pos) = bucket.iter().position(|p| p.node_id == peer.node_id) {
            // Move the existing peer to the front
            let p = bucket.remove(pos).unwrap();
            bucket.push_front(p);
        } else if bucket.len() < K_VALUE {
            bucket.push_front(peer);
        } else {
            // Here you might ping the last peer in the bucket to see if it's still alive
            // For now, we'll just ignore the new peer if the bucket is full
        }
    }

    pub fn find_closest_peers(&self, target_id: &[u8; 32]) -> Vec<Peer> {
        let mut peers: Vec<(u128, Peer)> = self
            .buckets
            .iter()
            .flat_map(|bucket| bucket.iter().cloned())
            .map(|peer| (xor_distance(&peer.node_id, target_id), peer))
            .collect();

        peers.sort_by_key(|(dist, _)| *dist);
        peers.into_iter().take(K_VALUE).map(|(_, peer)| peer).collect()
    }

    fn bucket_index(&self, node_id: &[u8; 32]) -> usize {
        let distance = xor_distance(&self.local_node_id, node_id);
        if distance == 0 {
            return 0;
        }
        // log2(distance)
        255 - distance.leading_zeros() as usize
    }
}

pub fn xor_distance(id1: &[u8; 32], id2: &[u8; 32]) -> u128 {
    let mut dist = [0u8; 16];
    for i in 0..16 {
        dist[i] = id1[i] ^ id2[i];
    }
    u128::from_be_bytes(dist)
}