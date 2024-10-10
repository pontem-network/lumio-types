use crate::ledger::Ledger;
use eyre::Error;
use futures::Stream;
use futures::StreamExt;
use lumio_types::{events::l2::EngineActions, Slot};
use std::sync::Arc;

pub struct ActionHandler<L, S> {
    ledger: Arc<L>,
    receiver: S,
    current_slot: u64,
}

impl<L, S> ActionHandler<L, S>
where
    L: Ledger + Send + Sync + 'static,
    S: Stream<Item = EngineActions> + Send + Sync + Unpin + 'static,
{
    pub fn new(ledger: Arc<L>, receiver: S) -> Self {
        Self {
            ledger,
            receiver,
            current_slot: 0,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        self.current_slot = self.ledger.get_committed_actions()?;

        while let Some(actions) = self.receiver.next().await {
            self.ensure_right_actions(actions.last_slot)?;
            let ledger = self.ledger.clone();
            tokio::task::spawn_blocking(move || ledger.apply_slot_actions(actions)).await??;
        }
        Ok(())
    }

    fn ensure_right_actions(&mut self, last_slot: Slot) -> Result<(), Error> {
        if self.current_slot != last_slot {
            return Err(Error::msg(format!(
                "ActionHandler: expected slot {}, got {}",
                self.current_slot, last_slot
            )));
        }
        self.current_slot = last_slot;
        Ok(())
    }
}
