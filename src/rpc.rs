use crate::{Address, Hash, PayloadId, Slot, UnixTimestamp};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxMint {
    pub account: Address,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PayloadAttrs {
    pub parent_payload: PayloadId,
    pub l1_slots: Vec<L1Slot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct L1Slot {
    pub slot: Slot,
    pub deposits: Vec<TxMint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Payload<T>
where
    T: BorshSerialize + BorshDeserialize + Clone + Debug + PartialEq,
{
    pub id: PayloadId,
    pub parent_payload: PayloadId,
    pub slots: Vec<SlotPayload<T>>,
    pub checkpoint: Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlotPayload<T>
where
    T: BorshSerialize + BorshDeserialize + Clone + Debug + PartialEq,
{
    pub slot: Slot,
    pub previous_blockhash: Hash,
    pub blockhash: Hash,
    pub block_time: Option<UnixTimestamp>,
    pub block_height: Option<u64>,
    pub txs: Vec<T>,
    pub bank_hash: Hash,
}
