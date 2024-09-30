use futures::prelude::*;
use lumio_p2p::{libp2p::Multiaddr, Config, JwtSecret, Node};
use lumio_types::rpc::{LumioEvents, PayloadStatus};

#[tokio::test]
async fn simple() {
    super::init();

    let jwt = rand::random::<JwtSecret>();
    let node1_addr = "/ip4/127.0.0.1/tcp/61023".parse::<Multiaddr>().unwrap();
    let (mut node1, runner) = Node::new(Config {
        listen_on: vec![node1_addr.clone()],
        bootstrap_addresses: vec![],
        jwt,
    })
    .unwrap();

    tokio::spawn(runner.run());

    let (mut node2, runner) = Node::new(Config {
        listen_on: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
        bootstrap_addresses: vec![node1_addr],
        jwt,
    })
    .unwrap();
    tokio::spawn(runner.run());

    let mut op_sol_events = node1.subscribe_lumio_op_sol_events().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    node2
        .send_lumio_op_sol(LumioEvents::SyncStatus((0, PayloadStatus::Pending)))
        .await
        .unwrap();

    assert!(matches!(
        op_sol_events.next().await,
        Some(LumioEvents::SyncStatus((0, PayloadStatus::Pending)))
    ));
}
