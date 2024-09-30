use std::{sync::Arc, time::Duration};

use crate::ledger::{Ledger, SlotArtifact};
use eyre::Error;
use lumio_types::Slot;
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    time::sleep,
};

const SLOT_SUB_CHANNEL_SIZE: usize = 13;

pub struct SlotSub<L> {
    slot: Slot,
    ledger: Arc<L>,
    sender: Sender<SlotArtifact>,
}

impl<L: Ledger> SlotSub<L> {
    pub fn new(slot: Slot, ledger: Arc<L>) -> (Self, Receiver<SlotArtifact>) {
        let (sender, rx) = channel(SLOT_SUB_CHANNEL_SIZE);
        (
            Self {
                slot,
                ledger,
                sender,
            },
            rx,
        )
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            if let Some(slot) = self.ledger.get_slot(self.slot).await? {
                self.sender.send(slot).await?;
                self.slot += 1;
            } else {
                // wait for the next slot
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
