use crate::ledger::Ledger;
use eyre::{eyre, Error};
use futures::Sink;
use futures::SinkExt;
use lumio_types::{events::l2::EngineActions, Slot};
use std::sync::Arc;

pub struct ActionPublisher<L, S> {
    slot: Slot,
    ledger: Arc<L>,
    sender: S,
}

impl<L, S> ActionPublisher<L, S>
where
    L: Ledger + Send + Sync + 'static,
    S: Sink<EngineActions> + Unpin + 'static,
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
            let actions =
                tokio::task::spawn_blocking(move || ledger.get_slot_actions(slot)).await??;
            self.slot += actions.slot as u64 + 1;
            self.sender
                .send(actions)
                .await
                .map_err(|_| eyre!("Failed to send slot payload"))?;
        }
    }
}
