use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::{h256::H256, Address};

use super::Transfer;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum EngineEvent {
    Sol(Transfer),
    Spl(SplEngineEvent),
    SendMessage(Message),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplEngineEvent {
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub from: Address,
    pub to: Address,
    pub data: Vec<u8>,
}
