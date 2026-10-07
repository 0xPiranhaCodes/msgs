use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub [u8; 16]);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerAddress {
    pub id: InstanceId,
    pub address: SocketAddr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkEvent {
    PeerJoined {
        id: InstanceId,
        identity: Identity,
    },
    PeerLeft {
        id: InstanceId,
        identity: Identity,
    },
    Message {
        id: InstanceId,
        identity: Identity,
        text: String,
        timestamp: u64,
    },
    Status(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserAction {
    Send(String),
    ListPeers,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEntry {
    Message {
        id: InstanceId,
        identity: Identity,
        text: String,
        timestamp: u64,
    },
    Presence {
        identity: Identity,
        joined: bool,
        timestamp: u64,
    },
    Status {
        text: String,
        timestamp: u64,
    },
}
