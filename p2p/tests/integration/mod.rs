use futures::prelude::*;
use lumio_p2p::{libp2p::Multiaddr, Config, JwtSecret, Node};
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayload, SlotPayloadWithEvents};

async fn start_nodes() -> impl Iterator<Item = Node> {
    let jwt = rand::random::<JwtSecret>();

    let bootstrap_addr = format!(
        "/ip4/127.0.0.1/tcp/{}",
        portpicker::pick_unused_port().unwrap()
    )
    .parse::<Multiaddr>()
    .unwrap();

    let rest = (1..).map({
        let bootstrap_addr = bootstrap_addr.clone();
        move |i| {
            let (node, runner) = Node::new(
                libp2p::identity::Keypair::generate_ed25519(),
                Config {
                    listen_on: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
                    bootstrap_addresses: vec![bootstrap_addr.clone()],
                    jwt: jwt.clone(),
                },
            )
            .unwrap();
            tokio::spawn(runner.run());
            tracing::info!("Finished with Node {i}");
            node
        }
    });

    std::iter::once({
        let (node, runner) = Node::new(
            libp2p::identity::Keypair::generate_ed25519(),
            Config {
                listen_on: vec![bootstrap_addr],
                bootstrap_addresses: vec![],
                jwt,
            },
        )
        .unwrap();

        tokio::spawn(runner.run());
        tracing::info!("Finished with Node 1");
        node
    })
    .chain(rest)
}

#[tokio::test]
async fn simple() {
    super::init();

    let nodes = start_nodes().await.take(2).collect::<Vec<_>>();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut op_sol_events = nodes[0].subscribe_lumio_op_sol_events().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    nodes[1]
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
async fn many_nodes() {
    super::init();

    let nodes = start_nodes().await.take(5).collect::<Vec<_>>();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let streams = futures::stream::iter(nodes[1..].iter())
        .then(|n| async {
            let s = n.subscribe_lumio_op_sol_events().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            s
        })
        .collect::<Vec<_>>()
        .await;

    nodes[0]
        .send_lumio_op_sol(SlotAttribute::new(
            1,
            vec![],
            Some((0, PayloadStatus::Pending)),
        ))
        .await
        .unwrap();

    for mut s in streams {
        assert_eq!(
            s.next().await,
            Some(SlotAttribute::new(
                1,
                vec![],
                Some((0, PayloadStatus::Pending)),
            ))
        );
    }
}

#[tokio::test]
async fn sub_since() {
    super::init();

    let nodes = start_nodes().await.take(2).collect::<Vec<_>>();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut op_sol_subs = nodes[0].handle_op_sol_since().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut op_sol_events = nodes[1].subscribe_op_sol_events_since(10).await.unwrap();
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

    let nodes = start_nodes().await.take(2).collect::<Vec<_>>();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut lumio_sol_subs = nodes[0].handle_lumio_sol_since().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut lumio_sol_events = nodes[1]
        .subscribe_lumio_op_sol_events_since(10)
        .await
        .unwrap();
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
