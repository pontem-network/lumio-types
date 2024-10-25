#![allow(unused_crate_dependencies)]

use lumio_rpc::{Engine, EngineConfig, Lumio, LumioConfig};
use poem::listener::TcpListener;
use poem::middleware::Tracing;
use poem::{EndpointExt, Server};

mod integration;

pub async fn start() -> (Lumio, Engine, Engine) {
    let new_port = || format!("localhost:{}", portpicker::pick_unused_port().unwrap());
    let jwt = rand::random();
    let (lumio, op_sol, op_move) = (new_port(), new_port(), new_port());

    let (sol, route) = Engine::new(EngineConfig {
        lumio: format!("http://{lumio}").parse().unwrap(),
        other_engine: format!("http://{op_move}").parse().unwrap(),
        jwt,
    });

    tokio::spawn(Server::new(TcpListener::bind(op_sol.to_string())).run(route.with(Tracing)));

    let (mv, route) = Engine::new(EngineConfig {
        lumio: format!("http://{lumio}").parse().unwrap(),
        other_engine: format!("http://{op_sol}").parse().unwrap(),
        jwt,
    });
    tokio::spawn(Server::new(TcpListener::bind(op_move.to_string())).run(route.with(Tracing)));

    let (lum, route) = Lumio::new(LumioConfig {
        op_sol: format!("http://{op_sol}").parse().unwrap(),
        op_move: format!("http://{op_move}").parse().unwrap(),
        jwt,
    });

    tokio::spawn(Server::new(TcpListener::bind(lumio.to_string())).run(route.with(Tracing)));

    // Wait to start listen
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    (lum, sol, mv)
}

pub fn init() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("lumio_rpc=trace,debug")
        .with_test_writer()
        .try_init();
}
