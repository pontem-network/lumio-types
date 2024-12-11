use crate::ledger::ApplyAbstraction;
use crate::skip_range::SkipRange;
use eyre::{Error, Result};
use futures::Stream;
use futures::StreamExt;
use lumio_types::events::SlotEvents;
use lumio_types::Slot;
use std::fmt::Debug;
use std::sync::Arc;

pub const SLOTS_TO_SKIP: u64 = 5 * 60 * 1000 / 400;

pub struct EventHandler<L, S> {
    ledger: Arc<L>,
    receiver: S,
    current_slot: u64,
}

impl<L, S, Event> EventHandler<L, S>
where
    L: ApplyAbstraction<Event> + Send + Sync + 'static,
    S: Stream<Item = Result<SlotEvents<Event>>> + Send + Sync + Unpin + 'static,
    Event: Debug + Send + Sync + 'static,
{
    pub fn new(ledger: L, receiver: S) -> Self {
        Self {
            ledger: Arc::new(ledger),
            receiver,
            current_slot: 0,
        }
    }

    fn ensure_right_slot(&mut self, slot: Slot) -> Result<(), Error> {
        if self.current_slot != slot {
            return Err(Error::msg(format!(
                "ActionHandler: expected slot {}, got {}",
                self.current_slot, slot
            )));
        }
        self.current_slot = slot + 1;
        Ok(())
    }

    pub async fn run(mut self) -> Result<(), Error> {
        let committed = self.ledger.get_committed_slot()?;

        self.current_slot = committed + 1;
        println!("Start event handler from:{}", committed);
        let mut skip_range = SkipRange::new(committed, SLOTS_TO_SKIP);

        while let Some(payload) = self.receiver.next().await {
            let payload = payload?;
            println!("handle slot :{:?}", payload);
            self.ensure_right_slot(payload.slot)?;

            if let Some((from, payload)) = skip_range.try_skip(payload) {
                let ledger = self.ledger.clone();
                tokio::task::spawn_blocking(move || ledger.apply_events(from, payload)).await??;
            }
        }

        Ok(())
    }
}
