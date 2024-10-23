use std::time::Duration;

use eyre::eyre;
use futures::{SinkExt, StreamExt};
use libp2p::{pnet::PreSharedKey, Multiaddr, PeerId};
use lumio_p2p::{
    simple_client::P2PClient,
    topics::{self, Topic},
    Config, Node, SolCommand,
};
use lumio_types::p2p::{SlotPayload, SlotPayloadWithEvents};
use rand::random;
use tokio::{join, time::sleep};

fn free_address() -> Multiaddr {
    format!(
        "/ip4/127.0.0.1/tcp/{}",
        portpicker::pick_unused_port().unwrap()
    )
    .parse()
    .unwrap()
}

async fn start_nodes(psk: PreSharedKey, count: usize) -> (Vec<PeerId>, Vec<Multiaddr>, Vec<Node>) {
    (0..count).fold(
        (Vec::new(), Vec::new(), Vec::new()),
        |(mut peer_ids, mut bootstrap_addresses, mut nodes), _| {
            let keypair = libp2p::identity::Keypair::generate_ed25519();
            peer_ids.push(keypair.public().to_peer_id());

            let listen_on = free_address();

            let (node, runner) = Node::new(
                keypair,
                Config {
                    listen_on: vec![listen_on.clone()],
                    bootstrap_addresses: bootstrap_addresses.clone(),
                    psk,
                },
            )
            .unwrap();
            tokio::spawn(runner.run());
            tracing::info!("Finished with Node {listen_on}");

            bootstrap_addresses.push(listen_on);
            nodes.push(node);

            (peer_ids, bootstrap_addresses, nodes)
        },
    )
}

#[tokio::test]
async fn test_client() {
    super::init();

    let mut payload = SlotPayloadWithEvents {
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

    let node_count = 4;
    let psk = PreSharedKey::new(rand::random());
    let (peer_ids, addresses, mut nodes) = start_nodes(psk, node_count).await;
    let main = nodes.remove(0);

    // Create a client and wait for all the peers to connect

    let mut client = P2PClient::new([free_address()], addresses.clone(), psk)
        .await
        .unwrap();

    assert_eq!(
        node_count,
        client.wait_peers_count(node_count).await.unwrap()
    );

    client.wait_peers(&peer_ids).await.unwrap();

    for node in nodes.iter() {
        let start_slot = random();
        join!(
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

                let item = node
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

    let start_slot = random();
    join!(
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

            client
                .publish(
                    topics::SolCommands.topic(),
                    SolCommand::SubEventsSince(topics::SolEventsSince(start_slot)),
                )
                .await
                .unwrap();
            let topic = topics::SolEventsSince(start_slot).topic();
            client.subscribe(topic.clone()).await.unwrap();

            assert_eq!(payload, client.next_message_with_decode().await.unwrap());
        }
    );

    // = = =
    // Receiving messages using the client
    // = = =
    let start_slot = random();
    payload.payload.slot = random();

    client.subscribe(topics::SolCommands.topic()).await.unwrap();

    join!(
        async {
            let SolCommand::SubEventsSince(topics::SolEventsSince(slot)) =
                client.next_message_with_decode().await.unwrap()
            else {
                panic!("A subscription message was expected");
            };

            assert_eq!(slot, start_slot);

            client
                .publish(topics::SolEventsSince(start_slot).topic(), payload.clone())
                .await
                .unwrap();
        },
        async {
            sleep(Duration::from_millis(200)).await;
            let item = main
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
