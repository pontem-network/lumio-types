use serde::{Deserialize, Serialize};

use crate::{Address, U256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Call {
    Move(MoveCall),
    Solana(SolanaCall),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveCall {
    pub signer: Address,
    pub module_address: Address,
    pub module: String,
    pub function: String,
    pub tp_args: Vec<String>,
    pub args: Vec<MoveValue>,
    pub attached_amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaCall {
    pub signer: Address,
    pub instruction: Vec<u8>,
    pub on_fail: Option<Box<MoveCall>>,
    pub os_success: Option<Box<MoveCall>>,
    pub attached_amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    U256(U256),
    Bool(bool),
    Address(Address),
    Vector(Vec<MoveValue>),
    Bytes(Vec<u8>),
    String(String),
}
