use std::future::Future;

use anyhow::Error;
use lumio_types::Slot;

use crate::SlotArtifact;

pub trait Ledger {
    /// Get the current slot.
    fn get_current_slot(&self) -> impl Future<Output = Result<Slot, Error>> + Send + 'static;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    /// If the slot with slot_id has not been produced yet, return None.
    fn get_slot(
        &mut self,
        slot_id: Slot,
    ) -> impl Future<Output = Result<Option<SlotArtifact>, Error>> + Send + 'static;
}
