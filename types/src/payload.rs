use crate::Address;
use super::Hash;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Payload {
    pub parent_block: Hash,
    pub l1_transactions: Vec<L1Transaction>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum L1Transaction {
    Deposit {
        /// The address to which the deposit is made.
        to: Address,
        /// The amount of the deposit.
        value: u64,
        /// If the l1_token is not specified, it means that the native token is used.
        l1_token: Option<Address>,
    },
    BindToken {
        /// The address to which the token is bound.
        token_l1_address: Address,
        /// The decimals of the token.
        decimals: u8,
        /// The symbol of the token.
        symbol: String,
        /// The name of the token.
        name: String,
    },
}
