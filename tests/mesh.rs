use msgs::{
    crypto::derive_room_keys,
    domain::{Identity, InstanceId, NetworkEvent, UserAction},
    net::mesh::{MeshConfig, MeshHandle},
};
use std::time::Duration;
use tokio::time::timeout;

fn config(id: u8, name: &str, room_domain: &str) -> MeshConfig {
    MeshConfig {
        identity: Identity {
            name: name.into(),
            email: format!("{name}@example.com"),
        },
        id: InstanceId([id; 16]),
        port: 0,
        keys: derive_room_keys(room_domain).unwrap(),
        seeds: vec![],
        multicast: false,
    }
}

#[tokio::test]
async fn peers_join_chat_leave_and_reconnect() {
    let b = MeshHandle::spawn(config(2, "grace", "hackerearth.com"))
        .await
        .unwrap();
    let mut b_events = b.subscribe();
    let mut a_config = config(1, "ada", "hackerearth.com");
    a_config.seeds.push(b.local_addr());
    let a = MeshHandle::spawn(a_config).await.unwrap();
    let mut a_events = a.subscribe();

    wait_for_join(&mut b_events, "ada").await;
    wait_for_join(&mut a_events, "grace").await;
    a.send(UserAction::Send("hello".into())).await.unwrap();
    let message = wait_for_message(&mut b_events).await;
    assert_eq!(message, ("ada".into(), "hello".into()));

    a.shutdown();
    wait_for_leave(&mut b_events, "ada").await;

    let mut a_config = config(1, "ada", "hackerearth.com");
    a_config.seeds.push(b.local_addr());
    let _replacement = MeshHandle::spawn(a_config).await.unwrap();
    wait_for_join(&mut b_events, "ada").await;
}

#[tokio::test]
async fn different_room_domain_never_joins() {
    let b = MeshHandle::spawn(config(2, "grace", "hackerearth.com"))
        .await
        .unwrap();
    let mut b_events = b.subscribe();
    let mut a_config = config(1, "ada", "example.com");
    a_config.seeds.push(b.local_addr());
    let _a = MeshHandle::spawn(a_config).await.unwrap();
    assert!(timeout(Duration::from_millis(400), b_events.recv())
        .await
        .is_err());
}

#[tokio::test]
async fn seed_introduces_peers_without_replaying_history() {
    let c = MeshHandle::spawn(config(3, "linus", "hackerearth.com"))
        .await
        .unwrap();
    let mut b_config = config(2, "grace", "hackerearth.com");
    b_config.seeds.push(c.local_addr());
    let b = MeshHandle::spawn(b_config).await.unwrap();
    let mut b_events = b.subscribe();
    wait_for_join(&mut b_events, "linus").await;

    b.send(UserAction::Send("before".into())).await.unwrap();
    let mut a_config = config(1, "ada", "hackerearth.com");
    a_config.seeds.push(b.local_addr());
    let a = MeshHandle::spawn(a_config).await.unwrap();
    let mut a_events = a.subscribe();
    wait_for_join(&mut a_events, "grace").await;
    wait_for_join(&mut a_events, "linus").await;

    assert!(
        timeout(Duration::from_millis(150), wait_for_message(&mut a_events))
            .await
            .is_err()
    );
    b.send(UserAction::Send("after".into())).await.unwrap();
    assert_eq!(
        wait_for_message(&mut a_events).await,
        ("grace".into(), "after".into())
    );
}

async fn wait_for_join(events: &mut tokio::sync::broadcast::Receiver<NetworkEvent>, name: &str) {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(NetworkEvent::PeerJoined { identity, .. }) = events.recv().await {
                if identity.name == name {
                    return;
                }
            }
        }
    })
    .await
    .unwrap();
}

async fn wait_for_leave(events: &mut tokio::sync::broadcast::Receiver<NetworkEvent>, name: &str) {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(NetworkEvent::PeerLeft { identity, .. }) = events.recv().await {
                if identity.name == name {
                    return;
                }
            }
        }
    })
    .await
    .unwrap();
}

async fn wait_for_message(
    events: &mut tokio::sync::broadcast::Receiver<NetworkEvent>,
) -> (String, String) {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(NetworkEvent::Message { identity, text, .. }) = events.recv().await {
                return (identity.name, text);
            }
        }
    })
    .await
    .unwrap()
}
