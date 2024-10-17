#![allow(unused_crate_dependencies)]
mod integration;
mod simple_client;

pub fn init() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("lumio_p2p=trace,info")
        .with_test_writer()
        .try_init();
}
