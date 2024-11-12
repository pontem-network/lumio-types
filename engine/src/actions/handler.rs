use crate::ledger::Ledger;
use crate::skip_range::SkipRange;
use crate::skip_range::SlotExt;
use eyre::{Error, Result};
use futures::Stream;
use futures::StreamExt;
use lumio_types::events::l2::EngineActions;
use lumio_types::Slot;
use std::sync::Arc;

pub const SLOTS_TO_SKIP: u64 = 5 * 60 * 1000 / 400;

pub struct ActionHandler<L, S> {
    ledger: Arc<L>,
    receiver: S,
    current_slot: u64,
}

impl<L, S> ActionHandler<L, S>
where
    L: Ledger + Send + Sync + 'static,
    S: Stream<Item = Result<EngineActions>> + Send + Sync + Unpin + 'static,
{
    pub fn new(ledger: Arc<L>, receiver: S) -> Self {
        Self {
            ledger,
            receiver,
            current_slot: 0,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        self.current_slot = self.ledger.get_committed_l1_slot()?;

        let mut skip_range = SkipRange::new(self.current_slot, SLOTS_TO_SKIP);
        while let Some(payload) = self.receiver.next().await {
            let payload = payload?;

            self.ensure_right_slot(payload.slot)?;

            if let Some((from, payload)) = skip_range.try_skip(payload) {
                let ledger = self.ledger.clone();
                tokio::task::spawn_blocking(move || ledger.apply_slot_actions(from, payload))
                    .await??;
            }
        }
        Ok(())
    }

    fn ensure_right_slot(&mut self, slot: Slot) -> Result<(), Error> {
        if self.current_slot + 1 != slot {
            return Err(Error::msg(format!(
                "SlotHandler: expected slot {}, got {}",
                self.current_slot + 1,
                slot
            )));
        }
        self.current_slot = slot;
        Ok(())
    }
}

impl SlotExt for EngineActions {
    fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    fn id(&self) -> Slot {
        self.slot
    }
}
