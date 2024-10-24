use std::time::Duration;

use eyre::eyre;
use futures::prelude::*;
use lumio_p2p::{libp2p::pnet::PreSharedKey, libp2p::Multiaddr, Config, Node};
use lumio_types::p2p::{SlotAttribute, SlotPayload, SlotPayloadWithEvents};
use rand::random;
use stream::StreamExt;
use tokio::{join, time::sleep};

async fn start_nodes() -> impl Iterator<Item = Node> {
    let psk = PreSharedKey::new(rand::random());

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
                    psk,
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
                psk,
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
async fn sub_since() {
    super::init();

    let start_slot = random();
    let payload = SlotPayloadWithEvents {
        payload: SlotPayload {
            slot: random(),
            previous_blockhash: Default::default(),
            blockhash: Default::default(),
            bank_hash: Default::default(),
            block_time: None,
            block_height: None,
            txs: vec![],
        },
        events: vec![],
    };

    let mut nodes = start_nodes().await;
    let main = nodes.next().unwrap();
    let client = nodes.next().unwrap();

    let (_, _) = join!(
        async {
            let (slot, mut stream) = main
                .handle_op_sol_since()
                .await
                .unwrap()
                .next()
                .await
                .unwrap();

            assert_eq!(start_slot, slot);
            stream
                .send(payload.clone())
                .await
                .map_err(|_| eyre!("Error sending"))
                .unwrap();
        },
        async {
            sleep(Duration::from_millis(200)).await;

            let item = client
                .subscribe_op_sol_events_since(start_slot)
                .await
                .unwrap()
                .next()
                .await
                .unwrap();
            assert_eq!(item, payload);
        }
    );
}

#[tokio::test]
async fn sub_lumio_since() {
    super::init();

    let start_slot = random();
    let payload = SlotAttribute {
        slot_id: 14,
        events: vec![],
    };

    let mut nodes = start_nodes().await;
    let main = nodes.next().unwrap();
    let client = nodes.next().unwrap();

    let (_, _) = join!(
        async {
            let (slot, mut stream) = main
                .handle_lumio_sol_since()
                .await
                .unwrap()
                .next()
                .await
                .unwrap();

            assert_eq!(start_slot, slot);
            stream
                .send(payload.clone())
                .await
                .map_err(|_| eyre!("Error sending"))
                .unwrap();
        },
        async {
            sleep(Duration::from_millis(200)).await;

            let item = client
                .subscribe_lumio_op_sol_events_since(start_slot)
                .await
                .unwrap()
                .next()
                .await
                .unwrap();
            assert_eq!(item, payload);
        }
    );
}
