use crate::{Address, Hash, PayloadId, Slot, UnixTimestamp, Version};
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct L2Info {
    /// Hash of l2 genesis block. May be used to identify the chain. l1 <-> l2 bridge may use it to verify the chain.
    pub genesis_hash: Hash,
    /// Submitted l1 slot
    pub l1_submitted_slot: Slot,
    /// Confirmed l1 slot
    pub l1_confirmed_slot: Slot,
    /// L2 version. slot on solana or ledger version on aptos.
    pub l2_version: Version,
}
