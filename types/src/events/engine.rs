use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::h256::H256;

use super::{call::Call, Transfer};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum EngineEvent {
    Sol(Transfer),
    Spl(SplEngineEvent),
    Call(Call),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplEngineEvent {
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}
