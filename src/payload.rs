use std::mem::size_of;

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::{
    base64::{Base64, Standard},
    formats::Unpadded,
};
use sha3::Digest;

use crate::{Hash, PayloadId, Slot, Transaction, UnixTimestamp};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Payload {
    pub id: PayloadId,
    pub parent_payload: PayloadId,
    pub slots: Vec<SlotPayload>,
    pub checkpoint: Hash,
}

impl Payload {
    pub fn hash(&self) -> Hash {
        let serialized = borsh::to_vec(self).expect("Never fails");
        let digest = sha3::Sha3_256::digest(serialized);

        Hash::from(<[u8; 32]>::try_from(&digest[..]).expect("Never fails"))
    }
}

#[serde_with::serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlotPayload {
    pub slot: Slot,
    pub previous_blockhash: Hash,
    pub blockhash: Hash,
    pub block_time: Option<UnixTimestamp>,
    pub block_height: Option<u64>,
    #[serde_as(as = "Vec<Base64<Standard, Unpadded>>")]
    pub txs: Vec<Transaction>,
    pub bank_hash: Hash,
}

impl SlotPayload {
    pub fn size(&self) -> usize {
        size_of::<Slot>()
            + size_of::<Hash>() * 3
            + size_of::<Option<UnixTimestamp>>()
            + size_of::<Option<u64>>()
            + self.txs.iter().map(|tx| tx.len()).sum::<usize>()
    }
}
