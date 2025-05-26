use super::Hash;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub parent_block: Hash,
    pub block_number: Hash,
    pub block: Vec<u8>,
    pub l2_transactions: Vec<L2Transaction>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum L2Transaction {
    Withdraw {
        /// The address to which the withdrawal is made.
        to: super::Address,
        /// The amount of the withdrawal.
        value: u64,
        /// If the l2_token is not specified, it means that the native token is used.
        l2_token: Option<super::Address>,
    },
}
