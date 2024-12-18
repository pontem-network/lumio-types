use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::h256::H256;

use super::Transfer;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum LumioEvent {
    Sol(Transfer),
    Spl(SplL1Event),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplL1Event {
    Bridge {
        l1_mint: H256,
        decimals: u8,
    },
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}
