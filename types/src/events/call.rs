use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::Address;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Call {
    Move(MoveCall),
    Solana(SolanaCall),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveCall {
    signer: Address,
    module_address: Address,
    module: String,
    function: String,
    tp_args: Vec<String>,
    args: Vec<MoveValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaCall {
    signer: Address,
    instruction: Vec<u8>,
    on_fail: Option<Box<MoveCall>>,
    os_success: Option<Box<MoveCall>>,
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
