use crate::ledger::Ledger;
use eyre::{eyre, Error};
use futures::Sink;
use futures::SinkExt;
use lumio_types::{p2p::SlotPayloadWithEvents, Slot};
use std::sync::Arc;

pub struct SlotSub<L, S> {
    slot: Slot,
    ledger: Arc<L>,
    sender: S,
}

impl<L, S> SlotSub<L, S>
where
    L: Ledger + Send + Sync + 'static,
    S: Sink<SlotPayloadWithEvents> + Unpin + 'static,
{
    pub fn new(slot: Slot, ledger: Arc<L>, sender: S) -> Self {
        Self {
            slot,
            ledger,
            sender,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let ledger = self.ledger.clone();
            let slot = self.slot;
            let slot = tokio::task::spawn_blocking(move || ledger.get_payload(slot)).await??;
            self.sender
                .send(slot)
                .await
                .map_err(|_| eyre!("Failed to send slot payload"))?;
            self.slot += 1;
        }
    }
}
