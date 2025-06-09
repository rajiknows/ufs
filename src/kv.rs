use std::{collections::HashMap, net::SocketAddr};

pub enum Roles{
    Leader,
    Follower,
    Candidate
}

mod rpc {
    pub enum RPC{
        AppendEntries,
        RequestVote,
        InstallSnapshot
    }

}

// under the whole key-value store we will use raft for consensus....
pub struct Node {
    addr: SocketAddr,
    store: HashMap<String, String>,
    peers : Vec<Node>,
    term : u64,
    commit_index : u64,
    voted_for : u64,
}


impl Node {
    pub fn new(addr: &SocketAddr) -> Self {
        Self {
            addr: *addr,
            store: HashMap::new(),
            peers: Vec::new()
        }
    }

    pub fn set(&mut self, key: String, val: String){
        *self.store.entry(key).and_modify(|has_value|has_value= *val).or_insert(val);
    }

    pub fn get(&self, key:String)->Option<String>{
        self.store.get(&key)
    }

}


/// alright this is my raw thoughts
/// 1 . ok so each node will be independent to each other ....
/// 2 . when we start a node it will have empty store , it will listen for the messages , if it gets a message then it folllows it
/// 3 . if not it starts the election and goes into the candidate mode ....
/// 4 .
