use crate::domain::{Identity, InstanceId, PeerAddress};
use serde::{Deserialize, Serialize};
use snow::{params::NoiseParams, Builder, HandshakeState, TransportState};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub const MAX_CHAT_BYTES: usize = 8 * 1024;
const MAX_FRAME_BYTES: usize = 16 * 1024;
const MAX_HANDSHAKE_BYTES: usize = 4 * 1024;
const NOISE_PATTERN: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerHello {
    pub version: u8,
    pub id: InstanceId,
    pub identity: Identity,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMessage {
    Chat(String),
    PeerList(Vec<PeerAddress>),
    Ping,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    message: WireMessage,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("protocol frame is too large")]
    TooLarge,
    #[error("unsupported protocol version")]
    Version,
    #[error("peer authentication failed")]
    Authentication,
    #[error("invalid protocol message")]
    Decode,
    #[error("connection closed")]
    Closed,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct NoiseConnection {
    stream: TcpStream,
    noise: TransportState,
}

impl NoiseConnection {
    pub async fn initiator(
        mut stream: TcpStream,
        psk: &[u8; 32],
        local: PeerHello,
    ) -> Result<(Self, PeerHello), ProtocolError> {
        let mut handshake = build_handshake(true, psk)?;
        let local_bytes = encode_hello(&local)?;
        write_handshake(&mut stream, &mut handshake, &local_bytes).await?;
        let remote_bytes = read_handshake(&mut stream, &mut handshake).await?;
        let remote = decode_hello(&remote_bytes)?;
        write_handshake(&mut stream, &mut handshake, &[]).await?;
        let noise = handshake
            .into_transport_mode()
            .map_err(|_| ProtocolError::Authentication)?;
        let mut connection = Self { stream, noise };
        let ack = connection.recv_plain().await?;
        if ack != b"msgs/v2/ready" {
            return Err(ProtocolError::Authentication);
        }
        Ok((connection, remote))
    }

    pub async fn responder(
        mut stream: TcpStream,
        psk: &[u8; 32],
        local: PeerHello,
    ) -> Result<(Self, PeerHello), ProtocolError> {
        let mut handshake = build_handshake(false, psk)?;
        let remote_bytes = read_handshake(&mut stream, &mut handshake).await?;
        let remote = decode_hello(&remote_bytes)?;
        let local_bytes = encode_hello(&local)?;
        write_handshake(&mut stream, &mut handshake, &local_bytes).await?;
        let final_payload = read_handshake(&mut stream, &mut handshake).await?;
        if !final_payload.is_empty() {
            return Err(ProtocolError::Authentication);
        }
        let noise = handshake
            .into_transport_mode()
            .map_err(|_| ProtocolError::Authentication)?;
        let mut connection = Self { stream, noise };
        connection.send_plain(b"msgs/v2/ready").await?;
        Ok((connection, remote))
    }

    pub async fn send(&mut self, message: &WireMessage) -> Result<(), ProtocolError> {
        validate_message(message)?;
        let plaintext = postcard::to_allocvec(&Envelope {
            version: 2,
            message: message.clone(),
        })
        .map_err(|_| ProtocolError::Decode)?;
        self.send_plain(&plaintext).await
    }

    pub async fn recv(&mut self) -> Result<WireMessage, ProtocolError> {
        let plaintext = self.recv_plain().await?;
        let envelope: Envelope =
            postcard::from_bytes(&plaintext).map_err(|_| ProtocolError::Decode)?;
        if envelope.version != 2 {
            return Err(ProtocolError::Version);
        }
        validate_message(&envelope.message)?;
        Ok(envelope.message)
    }

    async fn send_plain(&mut self, plaintext: &[u8]) -> Result<(), ProtocolError> {
        if plaintext.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        let mut encrypted = vec![0_u8; plaintext.len() + 64];
        let size = self
            .noise
            .write_message(plaintext, &mut encrypted)
            .map_err(|_| ProtocolError::Authentication)?;
        encrypted.truncate(size);
        write_outer(&mut self.stream, &encrypted).await
    }

    async fn recv_plain(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let encrypted = read_outer(&mut self.stream, MAX_FRAME_BYTES + 64).await?;
        let mut plaintext = vec![0_u8; encrypted.len()];
        let size = self
            .noise
            .read_message(&encrypted, &mut plaintext)
            .map_err(|_| ProtocolError::Authentication)?;
        plaintext.truncate(size);
        Ok(plaintext)
    }
}

fn validate_message(message: &WireMessage) -> Result<(), ProtocolError> {
    match message {
        WireMessage::Chat(text) if text.len() > MAX_CHAT_BYTES => Err(ProtocolError::TooLarge),
        WireMessage::PeerList(peers) if peers.len() > 128 => Err(ProtocolError::TooLarge),
        _ => Ok(()),
    }
}

fn build_handshake(initiator: bool, psk: &[u8; 32]) -> Result<HandshakeState, ProtocolError> {
    let params: NoiseParams = NOISE_PATTERN
        .parse()
        .map_err(|_| ProtocolError::Authentication)?;
    let keypair = Builder::new(params.clone())
        .generate_keypair()
        .map_err(|_| ProtocolError::Authentication)?;
    let builder = Builder::new(params)
        .local_private_key(&keypair.private)
        .psk(3, psk);
    if initiator {
        builder.build_initiator()
    } else {
        builder.build_responder()
    }
    .map_err(|_| ProtocolError::Authentication)
}

fn encode_hello(hello: &PeerHello) -> Result<Vec<u8>, ProtocolError> {
    if hello.version != 2 || hello.identity.name.is_empty() || hello.port == 0 {
        return Err(ProtocolError::Version);
    }
    postcard::to_allocvec(hello).map_err(|_| ProtocolError::Decode)
}

fn decode_hello(payload: &[u8]) -> Result<PeerHello, ProtocolError> {
    let hello: PeerHello = postcard::from_bytes(payload).map_err(|_| ProtocolError::Decode)?;
    if hello.version != 2 || hello.identity.name.is_empty() || hello.port == 0 {
        return Err(ProtocolError::Version);
    }
    Ok(hello)
}

async fn write_handshake(
    stream: &mut TcpStream,
    state: &mut HandshakeState,
    payload: &[u8],
) -> Result<(), ProtocolError> {
    let mut output = vec![0_u8; MAX_HANDSHAKE_BYTES];
    let size = state
        .write_message(payload, &mut output)
        .map_err(|_| ProtocolError::Authentication)?;
    output.truncate(size);
    write_outer(stream, &output).await
}

async fn read_handshake(
    stream: &mut TcpStream,
    state: &mut HandshakeState,
) -> Result<Vec<u8>, ProtocolError> {
    let input = read_outer(stream, MAX_HANDSHAKE_BYTES).await?;
    let mut output = vec![0_u8; MAX_HANDSHAKE_BYTES];
    let size = state
        .read_message(&input, &mut output)
        .map_err(|_| ProtocolError::Authentication)?;
    output.truncate(size);
    Ok(output)
}

async fn write_outer(stream: &mut TcpStream, payload: &[u8]) -> Result<(), ProtocolError> {
    if payload.len() > u32::MAX as usize {
        return Err(ProtocolError::TooLarge);
    }
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_outer(stream: &mut TcpStream, maximum: usize) -> Result<Vec<u8>, ProtocolError> {
    let length = stream.read_u32().await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            ProtocolError::Closed
        } else {
            ProtocolError::Io(error)
        }
    })? as usize;
    if length == 0 || length > maximum {
        return Err(ProtocolError::TooLarge);
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Identity, InstanceId};
    use tokio::net::{TcpListener, TcpStream};

    fn hello(id: u8) -> PeerHello {
        PeerHello {
            version: 2,
            id: InstanceId([id; 16]),
            identity: Identity {
                name: format!("peer-{id}"),
                email: format!("peer-{id}@example.com"),
            },
            port: 45_873,
        }
    }

    #[tokio::test]
    async fn noise_peers_exchange_encrypted_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let responder = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut connection, remote) = NoiseConnection::responder(stream, &[7; 32], hello(2))
                .await
                .unwrap();
            assert_eq!(remote.id, InstanceId([1; 16]));
            let message = connection.recv().await.unwrap();
            assert_eq!(message, WireMessage::Chat("hello".into()));
            connection.send(&WireMessage::Ping).await.unwrap();
        });

        let stream = TcpStream::connect(address).await.unwrap();
        let (mut connection, remote) = NoiseConnection::initiator(stream, &[7; 32], hello(1))
            .await
            .unwrap();
        assert_eq!(remote.id, InstanceId([2; 16]));
        connection
            .send(&WireMessage::Chat("hello".into()))
            .await
            .unwrap();
        assert_eq!(connection.recv().await.unwrap(), WireMessage::Ping);
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn wrong_psk_fails_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let responder = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            NoiseConnection::responder(stream, &[8; 32], hello(2)).await
        });
        let stream = TcpStream::connect(address).await.unwrap();
        assert!(NoiseConnection::initiator(stream, &[7; 32], hello(1))
            .await
            .is_err());
        assert!(responder.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn rejects_oversized_chat_before_writing() {
        let (mut first, _second) = connected_pair().await;
        let error = first
            .send(&WireMessage::Chat("x".repeat(MAX_CHAT_BYTES + 1)))
            .await
            .unwrap_err();
        assert!(matches!(error, ProtocolError::TooLarge));
    }

    async fn connected_pair() -> (NoiseConnection, NoiseConnection) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let responder = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            NoiseConnection::responder(stream, &[7; 32], hello(2))
                .await
                .unwrap()
                .0
        });
        let stream = TcpStream::connect(address).await.unwrap();
        let initiator = NoiseConnection::initiator(stream, &[7; 32], hello(1))
            .await
            .unwrap()
            .0;
        (initiator, responder.await.unwrap())
    }
}
