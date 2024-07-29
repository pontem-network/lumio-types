use anyhow::Error;
use async_trait::async_trait;
use lumio_types::Slot;

use crate::SlotArtifact;

#[async_trait]
pub trait Ledger {
    /// Get the current slot.
    async fn get_current_slot(&self) -> Result<Slot, Error>;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    /// If the slot with slot_id has not been produced yet, return None.
    async fn get_slot(&mut self, slot_id: Slot) -> Result<Option<SlotArtifact>, Error>;
}
