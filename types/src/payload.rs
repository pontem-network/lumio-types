use super::Hash;
use crate::{address::EthAddress, MoveAddress};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Payload {
    pub last_l1_block: Hash,
    pub last_l1_block_number: u64,
    pub parent_l2_block_number: u64,
    pub l1_transactions: Vec<L1Transaction>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum L1Transaction {
    Deposit {
        /// The address to which the deposit is made.
        to: MoveAddress,
        /// The amount of the deposit.
        value: u64,
        /// If the l1_token is not specified, it means that the native token is used.
        l1_token: Option<EthAddress>,
    },
    BindToken {
        /// The address to which the token is bound.
        token_l1_address: EthAddress,
        /// The decimals of the token.
        decimals: u8,
        /// The symbol of the token.
        symbol: String,
        /// The name of the token.
        name: String,
    },
}
