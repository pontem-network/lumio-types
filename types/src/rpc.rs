use serde::{Deserialize, Serialize};

use crate::events::l1::L1Event;
use crate::events::l2::L2Event;
use crate::payload::Payload;
use crate::{PayloadId, Slot};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PayloadAttrs {
    pub parent_payload: PayloadId,
    pub events: Vec<SlotEvents<L1Event>>,
    /// Max payload size in bytes.
    pub max_payload_size: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributesArtifact {
    pub payload: Payload,
    pub events: Vec<SlotEvents<L2Event>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotEvents<E> {
    pub slot: Slot,
    pub events: Vec<E>,
}
