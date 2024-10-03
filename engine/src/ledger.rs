use std::future::Future;

use eyre::Error;
use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
use lumio_types::{Hash, Slot};

pub trait Ledger {
    /// Get the current slot.
    fn get_current_slot(&self) -> Result<Slot, Error>;
    /// Get the committed l1 slot.
    fn get_committed_l1_slot(&self) -> Result<Slot, Error>;
    /// genesis hash
    fn genesis_hash(&self) -> Result<Hash, Error>;
    /// Get the version of the ledger.
    fn get_version(&self) -> Result<u64, Error>;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    /// If the slot with slot_id has not been produced yet, return None.
    fn get_slot(
        &self,
        slot_id: Slot,
    ) -> impl Future<Output = Result<Option<SlotPayloadWithEvents>, Error>> + Send;

    /// Apply events to the slot.
    fn apply_slot(
        &self,
        skip_from: Option<Slot>,
        slot: SlotAttribute,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}
