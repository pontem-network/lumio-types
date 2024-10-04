use futures::prelude::*;
use lumio_p2p::{libp2p::Multiaddr, Config, JwtSecret, Node};
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayload, SlotPayloadWithEvents};

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

#[tokio::test]
async fn simple() {
    super::init();

    let (node1, node2) = two_nodes().await;
    let mut op_sol_events = node1.subscribe_lumio_op_sol_events().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    node2
        .send_lumio_op_sol(SlotAttribute::new(
            1,
            vec![],
            Some((0, PayloadStatus::Pending)),
        ))
        .await
        .unwrap();

    assert_eq!(
        op_sol_events.next().await,
        Some(SlotAttribute::new(
            1,
            vec![],
            Some((0, PayloadStatus::Pending)),
        ))
    );
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
