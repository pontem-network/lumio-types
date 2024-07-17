use std::mem::size_of;

use base64::{decoded_len_estimate, encoded_len, engine::general_purpose::STANDARD_NO_PAD, Engine};
use borsh::{BorshDeserialize, BorshSerialize};
use derive_more::{Deref, From, Into};
use serde::{Deserialize, Deserializer, Serialize};
use sha3::Digest;

use crate::{Hash, PayloadId, Slot, UnixTimestamp};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlotPayload {
    pub slot: Slot,
    pub previous_blockhash: Hash,
    pub blockhash: Hash,
    pub block_time: Option<UnixTimestamp>,
    pub block_height: Option<u64>,
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

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, Deref, From, Into)]
pub struct Transaction(Vec<u8>);

impl Serialize for Transaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let len = encoded_len(self.0.len(), false).expect("transactions len <= 1234 bytes");
        let mut str = String::with_capacity(len);
        STANDARD_NO_PAD.encode_string(&self.0, &mut str);
        serializer.serialize_str(&str)
    }
}

impl<'d> Deserialize<'d> for Transaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'d>,
    {
        let str: String = Deserialize::deserialize(deserializer)?;
        let mut bytes = Vec::with_capacity(decoded_len_estimate(str.len()));
        STANDARD_NO_PAD
            .decode_vec(str.as_bytes(), &mut bytes)
            .map_err(serde::de::Error::custom)?;
        Ok(Transaction(bytes))
    }
}
