use lumio_rpc::{AttributesArtifact, SlotEvents};
use lumio_types::{
    events::l2::L2Event,
    h256::H256,
    payload::{Payload, SlotPayload},
    Slot,
};
use sha3::Digest as _;

pub struct ArtifactCollector {
    slots: Vec<SlotPayload>,
    events: Vec<SlotEvents<L2Event>>,
    max_payload_size: u32,
    parent_id: u64,
    size: usize,
}

impl ArtifactCollector {
    pub fn new(parent_id: u64, max_payload_size: u32) -> Self {
        Self {
            slots: vec![],
            parent_id,
            size: 0,
            max_payload_size,
            events: vec![],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn max_payload_size(&self) -> u32 {
        self.max_payload_size
    }

    pub fn collect(&mut self) -> AttributesArtifact {
        let id = self.slots.last().unwrap().slot;

        let mut hasher = sha3::Sha3_256::default();
        for slot in &self.slots {
            hasher.update(slot.bank_hash.as_ref());
        }

        let payload = Payload {
            parent_payload: self.parent_id,
            slots: std::mem::take(&mut self.slots),
            checkpoint: H256::from(
                <[u8; 32]>::try_from(&hasher.finalize()[..]).expect("Never fails"),
            ),
            id,
        };

        AttributesArtifact {
            payload,
            events: std::mem::take(&mut self.events),
        }
    }

    pub fn try_add(&mut self, slot: SlotPayload, events: SlotEvents<L2Event>) -> bool {
        let slot_size = slot.size();
        if self.size + slot_size > self.max_payload_size as usize {
            return false;
        }

        self.size += slot_size;
        self.slots.push(slot);
        self.events.push(events);
        true
    }

    pub fn slots_count(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn next_slot(&self) -> Slot {
        if self.is_empty() {
            if self.parent_id == 0 {
                0
            } else {
                self.parent_id + 1
            }
        } else {
            self.slots.last().unwrap().slot + 1
        }
    }
}
