use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::h256::H256;

use super::Bridge;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IntoStaticStr)]
pub enum L2Event {
    Withdrawal(Bridge),
    Spl(SplL2Event),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplL2Event {
    Bridged {
        l1_mint: H256,
        l2_mint: H256,
    },
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}
