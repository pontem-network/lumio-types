use eyre::Error;
use payload::Payload;
use tokio::sync::{mpsc, oneshot};

pub mod address;
pub mod block;
pub mod h256;
pub mod payload;

pub use crate::address::EthAddress;
pub type MoveAddress = h256::H256;
pub type Hash = h256::H256;

pub type BlockAccessSender = mpsc::Sender<BlockAccess>;
pub type BlockAccessReceiver = mpsc::Receiver<BlockAccess>;

pub enum BlockAccess {
    GetLatestBlock {
        response: oneshot::Sender<Result<block::Block, Error>>,
    },
    GetBlock {
        number: u64,
        response: oneshot::Sender<Result<block::Block, Error>>,
    },
}

pub type PayloadSender = mpsc::Sender<PayloadAccess>;
pub type PayloadReceiver = mpsc::Receiver<PayloadAccess>;

pub enum PayloadAccess {
    ApplyPayload {
        payload: Payload,
        response: oneshot::Sender<Result<Vec<block::Block>, Error>>,
    },
}
