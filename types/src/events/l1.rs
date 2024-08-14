use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::h256::H256;

use super::Bridge;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IntoStaticStr)]
pub enum L1Event {
    Deposit(Bridge),
    Spl(SplL1Event),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IntoStaticStr)]
pub enum SplL1Event {
    Bridge {
        l1_mint: H256,
        decimals: u8,
        metadata: Option<TokenMetadata>,
    },
    Transfer {
        l1_mint: H256,
        to: H256,
        amount: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}
