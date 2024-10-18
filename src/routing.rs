use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::net::SocketAddr;

const BUCKET_SIZE: usize = 20;
const ID_LENGTH_BITS: usize = 256;
type NodeId = [u8; 32];

#[derive(Clone, Debug)]
pub struct Node {
    id: NodeId,
    addr: SocketAddr,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

struct Bucket {
    nodes: Vec<Node>,
}

pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<Bucket>,
}

impl RoutingTable {
    pub fn new(local_id: NodeId) -> Self {
        RoutingTable {
            local_id,
            buckets: (0..ID_LENGTH_BITS)
                .map(|_| Bucket { nodes: Vec::new() })
                .collect(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        let bucket_index = self.bucket_index(&node.id);
        let bucket = &mut self.buckets[bucket_index];

        if let Some(existing) = bucket.nodes.iter_mut().find(|n| n.id == node.id) {
            *existing = node;
        } else if bucket.nodes.len() < BUCKET_SIZE {
            bucket.nodes.push(node);
        }
        // If bucket is full, we could implement node replacement strategy here
    }

    pub fn find_closest_nodes(&self, target: &NodeId, count: usize) -> Vec<Node> {
        let mut heap = BinaryHeap::with_capacity(count);

        for bucket in &self.buckets {
            for node in &bucket.nodes {
                let distance = xor_distance(&node.id, target);
                // Insert into the heap as Reverse to make it a min-heap
                heap.push(Reverse((distance, node.clone())));

                // If the heap exceeds the desired count, pop the farthest node
                if heap.len() > count {
                    heap.pop();
                }
            }
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|Reverse((_, node))| node)
            .collect()
    }

    fn bucket_index(&self, id: &NodeId) -> usize {
        ID_LENGTH_BITS - 1 - leading_zeroes(&xor_distance(&self.local_id, id))
    }
}

fn xor_distance(a: &NodeId, b: &NodeId) -> NodeId {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = a[i] ^ b[i];
    }
    result
}

fn leading_zeroes(id: &NodeId) -> usize {
    let mut bit_count = 0;
    for &byte in id {
        if byte == 0 {
            bit_count += 8;
        } else {
            bit_count += byte.leading_zeros() as usize;
            break;
        }
    }
    bit_count
}
