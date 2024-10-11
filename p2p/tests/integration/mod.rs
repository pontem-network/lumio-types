use std::time::Duration;

use futures::prelude::*;
use lumio_p2p::{libp2p::Multiaddr, Config, JwtSecret, Node};
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayload, SlotPayloadWithEvents};
use tokio::{runtime::Runtime, time::sleep};
use tracing::info;

async fn two_nodes() -> (Node, Node) {
    let jwt = rand::random::<JwtSecret>();
    let node1_addr = format!(
        "/ip4/127.0.0.1/tcp/{}",
        portpicker::pick_unused_port().unwrap()
    )
    .parse::<Multiaddr>()
    .unwrap();
    let (node1, runner) = Node::new(
        libp2p::identity::Keypair::generate_ed25519(),
        Config {
            listen_on: vec![node1_addr.clone()],
            bootstrap_addresses: vec![],
            jwt,
        },
    )
    .unwrap();

    tokio::spawn(runner.run());
    tracing::info!("Finished with Node 1");

    let (node2, runner) = Node::new(
        libp2p::identity::Keypair::generate_ed25519(),
        Config {
            listen_on: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
            bootstrap_addresses: vec![node1_addr],
            jwt,
        },
    )
    .unwrap();
    tokio::spawn(runner.run());
    tracing::info!("Finished with Node 2");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    (node1, node2)
}

const JWT: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

async fn two_nodes2(boot: &[u16], port1: u16, port2: u16) -> (Node, Node) {
    let mut bootstrap_addresses = boot
        .iter()
        .map(|port| {
            format!("/ip4/127.0.0.1/tcp/{port}")
                .parse::<Multiaddr>()
                .unwrap()
        })
        .collect::<Vec<_>>();

    let jwt = JwtSecret::new(JWT);
    let node1_addr = format!("/ip4/127.0.0.1/tcp/{port1}")
        .parse::<Multiaddr>()
        .unwrap();
    let (node1, runner) = Node::new(
        libp2p::identity::Keypair::generate_ed25519(),
        Config {
            listen_on: vec![node1_addr.clone()],
            bootstrap_addresses: bootstrap_addresses.clone(),
            jwt,
        },
    )
    .unwrap();
    bootstrap_addresses.push(node1_addr);

    tokio::spawn(runner.run());
    tracing::info!("Finished with Node 1");

    let (node2, runner) = Node::new(
        libp2p::identity::Keypair::generate_ed25519(),
        Config {
            listen_on: vec![format!("/ip4/127.0.0.1/tcp/{port2}")
                .parse::<Multiaddr>()
                .unwrap()],
            bootstrap_addresses,
            jwt,
        },
    )
    .unwrap();
    tokio::spawn(runner.run());
    tracing::info!("Finished with Node 2");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    (node1, node2)
}

#[tokio::test]
async fn simple() {
    super::init();

    let (node1, node2) = two_nodes2(&[], 50050, 50051).await;
    let (node3, node4) = two_nodes2(&[50050, 50051], 50052, 50053).await;
    // let (node1, node2) = two_nodes(50050, 50051, &[]).await;

    let _j = [node2, node3, node4]
        .into_iter()
        .enumerate()
        .map(|(n, node)| {
            let r = Runtime::new().unwrap();
            r.spawn(async move {
                let mut ev = node.subscribe_lumio_op_sol_events().await.unwrap();
                while let Some(s) = ev.next().await {
                    info!("{n}: {s:#?}");
                }
            });
            r
        })
        .collect::<Vec<_>>();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    for n in 1..u64::MAX {
        node1
            .send_lumio_op_sol(SlotAttribute::new(
                n,
                vec![],
                Some((n - 1, PayloadStatus::Pending)),
            ))
            .await
            .unwrap();

        sleep(Duration::from_secs(5)).await;
    }
}

#[tokio::test]
async fn sub_since() {
    super::init();

    let (node1, node2) = two_nodes().await;
    let mut op_sol_subs = node1.handle_op_sol_since().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut op_sol_events = node2.subscribe_op_sol_events_since(10).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let (slot, mut sink) = op_sol_subs.next().await.unwrap();
    assert_eq!(slot, 10);
    let payload = SlotPayloadWithEvents {
        payload: SlotPayload {
            slot: 14,
            previous_blockhash: Default::default(),
            blockhash: Default::default(),
            bank_hash: Default::default(),
            block_time: None,
            block_height: None,
            txs: vec![],
        },
        events: vec![],
    };
    let Ok(_) = sink.send(payload).await else {
        panic!()
    };

    assert_eq!(op_sol_events.next().await.unwrap().payload.slot, 14);
}

#[tokio::test]
async fn sub_lumio_since() {
    super::init();

    let (node1, node2) = two_nodes().await;
    let mut lumio_sol_subs = node1.handle_lumio_sol_since().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut lumio_sol_events = node2.subscribe_lumio_op_sol_events_since(10).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let (slot, mut sink) = lumio_sol_subs.next().await.unwrap();
    assert_eq!(slot, 10);
    let payload = SlotAttribute {
        slot_id: 14,
        events: vec![],
        sync_status: None,
    };
    let Ok(_) = sink.send(payload).await else {
        panic!()
    };

    assert_eq!(lumio_sol_events.next().await.unwrap().slot_id, 14);
}
