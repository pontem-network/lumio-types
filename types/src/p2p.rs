use crate::events::l2::EngineActions;
use crate::events::{l1::L1Event, l2::L2Event};
use crate::{Hash, Slot, Transaction, UnixTimestamp};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::{
    base64::{Base64, Standard},
    formats::Unpadded,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotPayloadWithEvents {
    pub payload: SlotPayload,
    pub events: Vec<L2Event>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotAttribute {
    pub slot_id: Slot,
    pub events: Vec<L1Event>,
    pub engines_events: Option<EngineActions>,
    pub sync_status: Option<(Slot, PayloadStatus)>,
}

impl SlotAttribute {
    pub fn new(
        slot_id: Slot,
        events: Vec<L1Event>,
        engines_events: Option<EngineActions>,
        sync_status: Option<(Slot, PayloadStatus)>,
    ) -> Self {
        Self {
            slot_id,
            events,
            engines_events,
            sync_status,
        }
    }

    pub fn id(&self) -> Slot {
        self.slot_id
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.sync_status.is_none()
    }
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
