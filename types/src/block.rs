use crate::EthAddress;

use super::Hash;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub parent_l2_block: u64,
    pub block: Hash,
    pub block_number: u64,
    pub timestamp: u64,
    pub last_l1_block: Hash,
    pub last_l1_block_number: u64,
    pub block_data: Vec<u8>,
    pub l2_transactions: Vec<L2Transaction>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum L2Transaction {
    Withdraw {
        /// The address to which the withdrawal is made.
        to: EthAddress,
        /// The amount of the withdrawal.
        value: u64,
        /// If the token_addr is not specified, it means that the native token is used.
        token_addr: Option<EthAddress>,
    },
}
