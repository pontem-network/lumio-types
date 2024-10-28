use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::{h256::H256, Slot};

use super::Bridge;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum L2Event {
    Withdrawal(Bridge),
    Spl(SplL2Event),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplL2Event {
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineActions {
    pub last_slot: Slot,
    pub slot: Slot,
    pub actions: Vec<EngineAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum EngineAction {
    Transfer(Bridge),
}
