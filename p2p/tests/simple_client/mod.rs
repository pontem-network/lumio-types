use std::time::Duration;

use futures::future::try_join_all;
use libp2p::{pnet::PreSharedKey, Multiaddr, PeerId};
use lumio_p2p::{
    simple_client::P2PClient,
    topics::{self, Topic},
    Config, Node,
};
use lumio_types::p2p::{PayloadStatus, SlotAttribute};
use tokio_stream::StreamExt;

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

    let psk = PreSharedKey::new(rand::random());
    let (peer_ids, addresses, nodes) = start_nodes(psk, 4).await;
    let node_count = peer_ids.len();

    // Create a client and wait for all the peers to connect

    let mut client = P2PClient::new([free_address()], addresses.clone(), psk)
        .await
        .unwrap();

    let auth_count = client.wait_peers_count(node_count).await.unwrap();
    assert_eq!(node_count, auth_count);

    client.wait_peers(&peer_ids).await.unwrap();

    // = = =
    // Receiving messages using the client
    // = = =

    client
        .subscribe(topics::LumioSolEvents.topic())
        .await
        .unwrap();

    // Nodes need time to process the subscription and start sending messages.
    tokio::time::sleep(Duration::from_millis(200)).await;

    for (n, nd) in nodes.iter().enumerate() {
        let n = n as u64;
        nd.send_lumio_op_sol(SlotAttribute::new(
            n + 1,
            vec![],
            Some((n, PayloadStatus::Pending)),
        ))
        .await
        .unwrap();
    }

    for _ in 0..nodes.len() {
        let m: SlotAttribute = client.next_message_with_decode().await.unwrap();
        (1..=node_count).contains(&(m.slot_id as usize));
    }
    // = = =
    // Sending messages using the client
    // = = =
    let node_sub = try_join_all(nodes.iter().map(|n| n.subscribe_lumio_op_sol_events()))
        .await
        .unwrap();
    // Nodes need time to process the subscription and start sending messages.
    client
        .wait_subscribe_peers(topics::LumioSolEvents.topic(), &peer_ids)
        .await
        .unwrap();

    client
        .publish(
            topics::LumioSolEvents.topic(),
            SlotAttribute::new(99, vec![], None),
        )
        .await
        .unwrap();

    for mut n in node_sub {
        let ev = n.next().await.unwrap();
        assert_eq!(99, ev.slot_id);
    }
}
