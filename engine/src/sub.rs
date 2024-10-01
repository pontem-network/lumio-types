use std::{sync::Arc, time::Duration};

use crate::ledger::Ledger;
use eyre::Error;
use lumio_types::{p2p::SlotArtifact, Slot};
use tokio::{sync::mpsc::Sender, time::sleep};

pub struct SlotSub<L> {
    slot: Slot,
    ledger: Arc<L>,
    sender: Sender<SlotArtifact>,
}

impl<L: Ledger> SlotSub<L> {
    pub fn new(slot: Slot, ledger: Arc<L>, sender: Sender<SlotArtifact>) -> Self {
        Self {
            slot,
            ledger,
            sender,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            if let Some(slot) = self.ledger.get_slot(self.slot).await? {
                self.sender.send(slot).await?;
                self.slot += 1;
            } else {
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
