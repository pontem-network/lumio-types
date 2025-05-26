use eyre::Error;
use payload::Payload;
use tokio::sync::{mpsc, oneshot};

pub mod block;
pub mod h256;
pub mod payload;

pub type Address = h256::H256;
pub type Hash = h256::H256;

pub type BlockAccessSender = mpsc::Sender<BlockAccess>;
pub type BlockAccessReceiver = mpsc::Receiver<BlockAccess>;

pub enum BlockAccess {
    GetBlock {
        payload_id: Hash,
        response: oneshot::Sender<Result<block::Block, Error>>,
    },
    GetNextBlock {
        id: Hash,
        response: oneshot::Sender<Result<Hash, Error>>,
    },
}

pub type PayloadSender = mpsc::Sender<PayloadAccess>;
pub type PayloadReceiver = mpsc::Receiver<PayloadAccess>;

pub enum PayloadAccess {
    ApplyPayload {
        payload: Payload,
        response: oneshot::Sender<Result<block::Block, Error>>,
    },
}
