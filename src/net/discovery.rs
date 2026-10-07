use crate::domain::{Identity, InstanceId};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::{HashSet, VecDeque},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{net::UdpSocket, sync::mpsc, time};
use tokio_util::sync::CancellationToken;

pub const CHAT_PORT: u16 = 45_873;
pub const MULTICAST_ADDR: &str = "239.255.77.77:45872";
const AAD: &[u8] = b"msgs/v2/discovery";
const MAX_PACKET: usize = 2_048;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub version: u8,
    pub id: InstanceId,
    pub identity: Identity,
    pub port: u16,
    pub timestamp: u64,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("invalid discovery packet size")]
    Size,
    #[error("discovery authentication failed")]
    Authentication,
    #[error("stale discovery announcement")]
    Stale,
    #[error("replayed discovery announcement")]
    Replay,
    #[error("unsupported discovery version")]
    Version,
    #[error("invalid discovery payload")]
    Decode,
}

#[derive(Default)]
pub struct ReplayCache {
    order: VecDeque<[u8; 24]>,
    seen: HashSet<[u8; 24]>,
}

impl ReplayCache {
    fn insert(&mut self, nonce: [u8; 24]) -> Result<(), DiscoveryError> {
        if !self.seen.insert(nonce) {
            return Err(DiscoveryError::Replay);
        }
        self.order.push_back(nonce);
        if self.order.len() > 1_024 {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }
        Ok(())
    }
}

pub fn seal_announcement<R>(
    key: &[u8; 32],
    announcement: &Announcement,
    rng: &mut R,
) -> Result<Vec<u8>, DiscoveryError>
where
    R: RngCore + CryptoRng,
{
    let plaintext = postcard::to_allocvec(announcement).map_err(|_| DiscoveryError::Decode)?;
    let mut nonce = [0_u8; 24];
    rng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| DiscoveryError::Authentication)?;
    let mut packet = Vec::with_capacity(nonce.len() + ciphertext.len());
    packet.extend_from_slice(&nonce);
    packet.extend_from_slice(&ciphertext);
    if packet.len() > MAX_PACKET {
        return Err(DiscoveryError::Size);
    }
    Ok(packet)
}

pub fn open_announcement(
    key: &[u8; 32],
    packet: &[u8],
    now: u64,
    replay: &mut ReplayCache,
) -> Result<Announcement, DiscoveryError> {
    if !(40..=MAX_PACKET).contains(&packet.len()) {
        return Err(DiscoveryError::Size);
    }
    let nonce: [u8; 24] = packet[..24].try_into().map_err(|_| DiscoveryError::Size)?;
    let cipher = XChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &packet[24..],
                aad: AAD,
            },
        )
        .map_err(|_| DiscoveryError::Authentication)?;
    let announcement: Announcement =
        postcard::from_bytes(&plaintext).map_err(|_| DiscoveryError::Decode)?;
    if announcement.version != 2 {
        return Err(DiscoveryError::Version);
    }
    if now.saturating_sub(announcement.timestamp) > 20 || announcement.timestamp > now + 5 {
        return Err(DiscoveryError::Stale);
    }
    replay.insert(nonce)?;
    Ok(announcement)
}

pub fn parse_seeds(values: &[String]) -> anyhow::Result<Vec<SocketAddr>> {
    values
        .iter()
        .map(|value| {
            if let Ok(address) = value.parse::<SocketAddr>() {
                return Ok(address);
            }
            let ip: IpAddr = value
                .parse()
                .map_err(|error| anyhow::anyhow!("invalid peer {value:?}: {error}"))?;
            Ok(SocketAddr::new(ip, CHAT_PORT))
        })
        .collect()
}

pub fn multicast_socket() -> anyhow::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.set_multicast_loop_v4(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 45_872).into())?;
    socket.join_multicast_v4(&Ipv4Addr::new(239, 255, 77, 77), &Ipv4Addr::UNSPECIFIED)?;
    Ok(UdpSocket::from_std(socket.into())?)
}

pub async fn run_multicast(
    cancel: CancellationToken,
    key: [u8; 32],
    mut local: Announcement,
    found: mpsc::Sender<(Announcement, SocketAddr)>,
) -> anyhow::Result<()> {
    let socket = multicast_socket()?;
    let target: SocketAddr = MULTICAST_ADDR.parse()?;
    let mut ticker = time::interval(Duration::from_secs(5));
    let mut replay = ReplayCache::default();
    let mut buffer = vec![0_u8; MAX_PACKET + 1];
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = ticker.tick() => {
                local.timestamp = unix_time();
                let packet = {
                    let mut rng = rand::thread_rng();
                    seal_announcement(&key, &local, &mut rng)?
                };
                socket.send_to(&packet, target).await?;
            }
            received = socket.recv_from(&mut buffer) => {
                let (size, source) = received?;
                if let Ok(remote) = open_announcement(&key, &buffer[..size], unix_time(), &mut replay) {
                    if remote.id != local.id {
                        let address = SocketAddr::new(source.ip(), remote.port);
                        if found.send((remote, address)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    fn announcement(timestamp: u64) -> Announcement {
        Announcement {
            version: 2,
            id: InstanceId([1; 16]),
            identity: Identity {
                name: "Ada".into(),
                email: "ada@example.com".into(),
            },
            port: 45_873,
            timestamp,
        }
    }

    #[test]
    fn encrypted_announcement_round_trips_once() {
        let key = [7; 32];
        let mut rng = StdRng::seed_from_u64(1);
        let packet = seal_announcement(&key, &announcement(1_000), &mut rng).unwrap();
        let mut replay = ReplayCache::default();
        let opened = open_announcement(&key, &packet, 1_005, &mut replay).unwrap();
        assert_eq!(opened, announcement(1_000));
        assert!(matches!(
            open_announcement(&key, &packet, 1_005, &mut replay),
            Err(DiscoveryError::Replay)
        ));
    }

    #[test]
    fn rejects_wrong_key_tampering_and_stale_packets() {
        let mut rng = StdRng::seed_from_u64(2);
        let packet = seal_announcement(&[1; 32], &announcement(1_000), &mut rng).unwrap();
        assert!(open_announcement(&[2; 32], &packet, 1_000, &mut ReplayCache::default()).is_err());
        let mut tampered = packet.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(
            open_announcement(&[1; 32], &tampered, 1_000, &mut ReplayCache::default()).is_err()
        );
        assert!(matches!(
            open_announcement(&[1; 32], &packet, 1_100, &mut ReplayCache::default()),
            Err(DiscoveryError::Stale)
        ));
    }

    #[test]
    fn parses_seed_default_and_explicit_ports() {
        let seeds = parse_seeds(&["10.0.0.4".into(), "10.0.0.5:5000".into()]).unwrap();
        assert_eq!(seeds[0].to_string(), "10.0.0.4:45873");
        assert_eq!(seeds[1].to_string(), "10.0.0.5:5000");
    }

    #[tokio::test]
    async fn multicast_socket_binds_discovery_port() {
        let socket = multicast_socket().unwrap();
        assert_eq!(socket.local_addr().unwrap().port(), 45_872);
    }
}
