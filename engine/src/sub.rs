use crate::ledger::Ledger;
use eyre::{eyre, Error};
use futures::Sink;
use futures::SinkExt;
use lumio_types::{p2p::SlotPayloadWithEvents, Slot};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub struct SlotSub<L, S> {
    slot: Slot,
    ledger: Arc<L>,
    sender: S,
    slot_time: Duration,
}

impl<L: Ledger, S: Sink<SlotPayloadWithEvents> + Unpin + 'static> SlotSub<L, S> {
    pub fn new(slot: Slot, ledger: Arc<L>, sender: S, slot_time: Duration) -> Self {
        Self {
            slot,
            ledger,
            sender,
            slot_time,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            if let Some(slot) = self.ledger.get_slot(self.slot).await? {
                self.sender
                    .send(slot)
                    .await
                    .map_err(|_| eyre!("Failed to send slot payload"))?;
                self.slot += 1;
            } else {
                sleep(self.slot_time).await;
            }
        }
    }
}
