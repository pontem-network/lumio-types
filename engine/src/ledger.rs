use eyre::Error;
use lumio_types::events::l2::EngineActions;
use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
use lumio_types::{Hash, Slot};

pub trait Ledger {
    /// Get the committed l1 slot.
    fn get_committed_l1_slot(&self) -> Result<Slot, Error>;
    /// genesis hash
    fn genesis_hash(&self) -> Result<Hash, Error>;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    /// If the slot with slot_id has not been produced yet, return None.
    fn get_slot(&self, slot_id: Slot) -> Result<SlotPayloadWithEvents, Error>;
    /// Apply events to the slot.
    fn apply_slot(&self, skip_from: Option<Slot>, slot: SlotAttribute) -> Result<(), Error>;
    /// Get the committed l1 slot.
    fn get_committed_actions(&self) -> Result<Slot, Error>;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    fn get_slot_actions(&self, slot_id: Slot) -> Result<EngineActions, Error>;
    /// Apply actions to the slot.
    fn apply_slot_actions(&self, actions: EngineActions) -> Result<(), Error>;
}
