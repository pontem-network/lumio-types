use crate::events::SlotEvents;
use crate::events::{engine::EngineEvent, lumio::LumioEvent};
use crate::{Hash, Slot, Transaction, UnixTimestamp};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::{
    base64::{Base64, Standard},
    formats::Unpadded,
};

pub type LumioEvents = SlotEvents<LumioEvent>;

pub type EngineEvents = SlotEvents<EngineEvent>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotPayloadWithEvents {
    pub payload: SlotPayload,
    pub events: EngineEvents,
}

// Copied from here: https://docs.optimism.io/builders/app-developers/transactions/statuses
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PayloadStatus {
    /// By default
    #[default]
    Pending,
    /// Given away to sequencer
    Unsafe,
    /// Sent to L1/DA
    Safe,
    /// Finalized on L1/DA
    L1Finalized,
    /// Wasn't disputed
    Finalized,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
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
