use crate::{
    crypto::RoomKeys,
    domain::{Identity, InstanceId, NetworkEvent, PeerAddress, UserAction},
    net::discovery::{run_multicast, Announcement},
    protocol::{NoiseConnection, PeerHello, WireMessage},
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
    time,
};
use tokio_util::sync::CancellationToken;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct MeshConfig {
    pub identity: Identity,
    pub id: InstanceId,
    pub port: u16,
    pub keys: RoomKeys,
    pub seeds: Vec<SocketAddr>,
    pub multicast: bool,
}

#[derive(Clone)]
pub struct MeshHandle {
    actions: mpsc::Sender<UserAction>,
    events: broadcast::Sender<NetworkEvent>,
    cancel: CancellationToken,
    local_addr: SocketAddr,
}

impl MeshHandle {
    pub async fn spawn(mut config: MeshConfig) -> Result<Self> {
        let listener = TcpListener::bind((IpAddr::from([0, 0, 0, 0]), config.port))
            .await
            .context("listen for peers")?;
        let local_addr = listener.local_addr()?;
        config.port = local_addr.port();
        let (actions, action_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (events, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (internal_tx, internal_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let cancel = CancellationToken::new();
        tokio::spawn(run_manager(
            config,
            listener,
            action_rx,
            internal_tx,
            internal_rx,
            events.clone(),
            cancel.clone(),
        ));
        Ok(Self {
            actions,
            events,
            cancel,
            local_addr,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<NetworkEvent> {
        self.events.subscribe()
    }

    pub async fn send(&self, action: UserAction) -> Result<()> {
        self.actions.send(action).await.context("mesh has stopped")
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

struct PeerRecord {
    identity: Identity,
    address: SocketAddr,
    preferred: bool,
    token: u64,
    outgoing: mpsc::Sender<WireMessage>,
}

enum Internal {
    Connected {
        connection: Box<NoiseConnection>,
        hello: PeerHello,
        address: SocketAddr,
        outgoing: bool,
    },
    Incoming {
        id: InstanceId,
        token: u64,
        message: WireMessage,
    },
    Disconnected {
        id: InstanceId,
        token: u64,
    },
    ConnectFailed {
        address: SocketAddr,
        error: String,
    },
    DiscoveryFailed(String),
}

async fn run_manager(
    config: MeshConfig,
    listener: TcpListener,
    mut actions: mpsc::Receiver<UserAction>,
    internal_tx: mpsc::Sender<Internal>,
    mut internal_rx: mpsc::Receiver<Internal>,
    events: broadcast::Sender<NetworkEvent>,
    cancel: CancellationToken,
) {
    let (discovery_tx, mut discovery_rx) = mpsc::channel(CHANNEL_CAPACITY);
    for address in &config.seeds {
        spawn_connect(*address, &config, internal_tx.clone());
    }
    if config.multicast {
        let announcement = Announcement {
            version: 2,
            id: config.id,
            identity: config.identity.clone(),
            port: config.port,
            timestamp: 0,
        };
        let key = *config.keys.discovery;
        let discovery_cancel = cancel.clone();
        let discovery_internal = internal_tx.clone();
        tokio::spawn(async move {
            if let Err(error) =
                run_multicast(discovery_cancel, key, announcement, discovery_tx).await
            {
                let _ = discovery_internal
                    .send(Internal::DiscoveryFailed(error.to_string()))
                    .await;
            }
        });
    }

    let mut peers: HashMap<InstanceId, PeerRecord> = HashMap::new();
    let mut next_token = 1_u64;
    let mut retry = time::interval(Duration::from_secs(5));
    retry.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            Some((_announcement, address)) = discovery_rx.recv() => {
                if !peers.values().any(|peer| peer.address == address) {
                    spawn_connect(address, &config, internal_tx.clone());
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, source)) => {
                        spawn_accept(stream, source, &config, internal_tx.clone());
                    }
                    Err(error) => {
                        let _ = events.send(NetworkEvent::Status(format!("peer listener failed: {error}")));
                    }
                }
            }
            Some(action) = actions.recv() => {
                match action {
                    UserAction::Send(text) => {
                        for peer in peers.values() {
                            let _ = peer.outgoing.try_send(WireMessage::Chat(text.clone()));
                        }
                    }
                    UserAction::ListPeers => {
                        let _ = events.send(NetworkEvent::Status(format!("{} peer(s) connected", peers.len())));
                    }
                    UserAction::Quit => break,
                }
            }
            Some(message) = internal_rx.recv() => {
                match message {
                    Internal::Connected { connection, hello, address, outgoing } => {
                        if hello.id == config.id {
                            continue;
                        }
                        let preferred = (config.id < hello.id) == outgoing;
                        let replace = match peers.get(&hello.id) {
                            Some(existing) => !existing.preferred && preferred,
                            None => true,
                        };
                        if !replace {
                            continue;
                        }
                        let existing_addresses: Vec<PeerAddress> = peers
                            .iter()
                            .filter(|(id, _)| **id != hello.id)
                            .map(|(id, peer)| PeerAddress { id: *id, address: peer.address })
                            .collect();
                        let was_present = peers.contains_key(&hello.id);
                        let token = next_token;
                        next_token += 1;
                        let (outgoing_tx, outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
                        if !existing_addresses.is_empty() {
                            let _ = outgoing_tx.try_send(WireMessage::PeerList(existing_addresses));
                        }
                        for (id, peer) in &peers {
                            if *id != hello.id {
                                let _ = peer.outgoing.try_send(WireMessage::PeerList(vec![
                                    PeerAddress { id: hello.id, address }
                                ]));
                            }
                        }
                        peers.insert(hello.id, PeerRecord {
                            identity: hello.identity.clone(),
                            address,
                            preferred,
                            token,
                            outgoing: outgoing_tx,
                        });
                        tokio::spawn(run_peer(
                            hello.id,
                            token,
                            *connection,
                            outgoing_rx,
                            internal_tx.clone(),
                        ));
                        if !was_present {
                            let _ = events.send(NetworkEvent::PeerJoined {
                                id: hello.id,
                                identity: hello.identity,
                            });
                        }
                    }
                    Internal::Incoming { id, token, message } => {
                        if peers.get(&id).is_none_or(|peer| peer.token != token) {
                            continue;
                        }
                        match message {
                            WireMessage::Chat(text) => {
                                if let Some(peer) = peers.get(&id) {
                                    let _ = events.send(NetworkEvent::Message {
                                        id,
                                        identity: peer.identity.clone(),
                                        text,
                                        timestamp: now(),
                                    });
                                }
                            }
                            WireMessage::PeerList(addresses) => {
                                for introduced in addresses {
                                    if introduced.id != config.id && !peers.contains_key(&introduced.id) {
                                        spawn_connect(introduced.address, &config, internal_tx.clone());
                                    }
                                }
                            }
                            WireMessage::Ping => {}
                        }
                    }
                    Internal::Disconnected { id, token } => {
                        if peers.get(&id).is_some_and(|peer| peer.token == token) {
                            if let Some(peer) = peers.remove(&id) {
                                let _ = events.send(NetworkEvent::PeerLeft {
                                    id,
                                    identity: peer.identity,
                                });
                            }
                        }
                    }
                    Internal::ConnectFailed { address, error } => {
                        let _ = events.send(NetworkEvent::Status(format!("peer {address} unavailable: {error}")));
                    }
                    Internal::DiscoveryFailed(error) => {
                        let _ = events.send(NetworkEvent::Status(format!(
                            "multicast discovery unavailable: {error}; use --peer <VPN-IP>"
                        )));
                    }
                }
            }
            _ = retry.tick() => {
                for address in &config.seeds {
                    if !peers.values().any(|peer| peer.address == *address) {
                        spawn_connect(*address, &config, internal_tx.clone());
                    }
                }
            }
        }
    }
}

fn spawn_connect(address: SocketAddr, config: &MeshConfig, internal: mpsc::Sender<Internal>) {
    let hello = local_hello(config);
    let psk = *config.keys.noise_psk;
    tokio::spawn(async move {
        let result = async {
            let stream = time::timeout(Duration::from_secs(3), TcpStream::connect(address))
                .await
                .context("connect timeout")??;
            let source = stream.peer_addr()?;
            let (connection, remote) = NoiseConnection::initiator(stream, &psk, hello).await?;
            let address = SocketAddr::new(source.ip(), remote.port);
            anyhow::Ok(Internal::Connected {
                connection: Box::new(connection),
                hello: remote,
                address,
                outgoing: true,
            })
        }
        .await;
        let message = result.unwrap_or_else(|error| Internal::ConnectFailed {
            address,
            error: error.to_string(),
        });
        let _ = internal.send(message).await;
    });
}

fn spawn_accept(
    stream: TcpStream,
    source: SocketAddr,
    config: &MeshConfig,
    internal: mpsc::Sender<Internal>,
) {
    let hello = local_hello(config);
    let psk = *config.keys.noise_psk;
    tokio::spawn(async move {
        if let Ok((connection, remote)) = NoiseConnection::responder(stream, &psk, hello).await {
            let address = SocketAddr::new(source.ip(), remote.port);
            let _ = internal
                .send(Internal::Connected {
                    connection: Box::new(connection),
                    hello: remote,
                    address,
                    outgoing: false,
                })
                .await;
        }
    });
}

async fn run_peer(
    id: InstanceId,
    token: u64,
    mut connection: NoiseConnection,
    mut outgoing: mpsc::Receiver<WireMessage>,
    internal: mpsc::Sender<Internal>,
) {
    loop {
        tokio::select! {
            message = outgoing.recv() => {
                match message {
                    Some(message) if connection.send(&message).await.is_ok() => {}
                    _ => break,
                }
            }
            result = connection.recv() => {
                match result {
                    Ok(message) => {
                        if internal.send(Internal::Incoming { id, token, message }).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
    let _ = internal.send(Internal::Disconnected { id, token }).await;
}

fn local_hello(config: &MeshConfig) -> PeerHello {
    PeerHello {
        version: 2,
        id: config.id,
        identity: config.identity.clone(),
        port: config.port,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
