use crate::ledger::{Ledger, SlotAttribute};
use eyre::Error;
use lumio_types::Slot;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

const SLOT_HANDLER_CHANNEL_SIZE: usize = 42;
const SLOTS_TO_SKIP: u64 = 5 * 60 * 1000 / 400;

pub struct SlotHandler<L> {
    ledger: Arc<L>,
    receiver: Receiver<SlotAttribute>,
    current_slot: u64,
}

impl<L: Ledger + Send + Sync + 'static> SlotHandler<L> {
    pub fn new(ledger: Arc<L>) -> (Self, Sender<SlotAttribute>) {
        let (sender, receiver) = channel(SLOT_HANDLER_CHANNEL_SIZE);
        (
            Self {
                ledger,
                receiver,
                current_slot: 0,
            },
            sender,
        )
    }

    pub async fn run(mut self) -> Result<(), Error> {
        self.current_slot = self.ledger.get_committed_l1_slot()?;

        let mut skip_range = SkipRange::new(self.current_slot, SLOTS_TO_SKIP);
        loop {
            if let Some(payload) = self.receiver.recv().await {
                self.ensure_right_slot(payload.id())?;
                if let Some((from, payload)) = skip_range.try_skip(payload) {
                    self.ledger.apply_slot(from, payload).await?;
                }
            } else {
                return Err(Error::msg("SlotHandler: channel closed"));
            }
        }
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

struct SkipRange {
    from: u64,
    max_slots_to_skip: u64,
    skipped: u64,
}

impl SkipRange {
    fn new(from: u64, max_slots_to_skip: u64) -> Self {
        Self {
            from,
            max_slots_to_skip,
            skipped: 0,
        }
    }

    fn try_skip(&mut self, payload: SlotAttribute) -> Option<(Option<Slot>, SlotAttribute)> {
        if payload.is_empty() {
            if self.skipped >= self.max_slots_to_skip {
                let from = Some(self.from);
                self.reset(payload.id());
                Some((from, payload))
            } else {
                self.skipped += 1;
                None
            }
        } else {
            let from = if self.skipped == 0 {
                None
            } else {
                Some(self.from)
            };
            self.reset(payload.id());
            Some((from, payload))
        }
    }

    fn reset(&mut self, from: u64) {
        self.skipped = 0;
        self.from = from;
    }
}

#[cfg(test)]
mod tests {
    use lumio_types::{
        events::{l1::L1Event, Bridge},
        h256::H256,
    };

    use super::*;

    impl SlotAttribute {
        fn empty(slot_id: u64) -> Self {
            Self::new(slot_id, vec![])
        }

        fn with_events(slot_id: u64) -> Self {
            Self::new(
                slot_id,
                vec![L1Event::Deposit(Bridge {
                    account: H256::default(),
                    amount: 2,
                })],
            )
        }
    }

    #[test]
    fn test_skip_range_new() {
        let skip_range = SkipRange::new(10, 5);
        assert_eq!(skip_range.from, 10);
        assert_eq!(skip_range.max_slots_to_skip, 5);
        assert_eq!(skip_range.skipped, 0);
    }

    #[test]
    fn test_skip_range_try_skip_empty_payload() {
        let mut skip_range = SkipRange::new(10, 5);
        let payload = SlotAttribute::empty(11);
        assert_eq!(skip_range.try_skip(payload.clone()), None);
        assert_eq!(skip_range.skipped, 1);
    }

    #[test]
    fn test_skip_range_try_skip_non_empty_payload() {
        let mut skip_range = SkipRange::new(10, 5);
        let payload = SlotAttribute::with_events(11);
        let result = skip_range.try_skip(payload.clone());
        assert!(result.is_some());
        let (from, returned_payload) = result.unwrap();
        assert_eq!(from, None);
        assert_eq!(returned_payload, payload);
        assert_eq!(skip_range.skipped, 0);
        assert_eq!(skip_range.from, 11);
    }

    #[test]
    fn test_skip_range_try_skip_max_skipped() {
        let mut skip_range = SkipRange::new(10, 2);
        let payload1 = SlotAttribute::empty(11);
        let payload2 = SlotAttribute::empty(12);
        let payload3 = SlotAttribute::empty(13);

        assert_eq!(skip_range.try_skip(payload1.clone()), None);
        assert_eq!(skip_range.skipped, 1);

        assert_eq!(skip_range.try_skip(payload2.clone()), None);
        assert_eq!(skip_range.skipped, 2);

        let result = skip_range.try_skip(payload3.clone());
        assert!(result.is_some());
        let (from, returned_payload) = result.unwrap();
        assert_eq!(from, Some(10));
        assert_eq!(returned_payload, payload3);
        assert_eq!(skip_range.skipped, 0);
        assert_eq!(skip_range.from, 13);
    }

    #[test]
    fn test_skip_range_reset() {
        let mut skip_range = SkipRange::new(10, 5);
        skip_range.skipped = 3;
        skip_range.reset(15);
        assert_eq!(skip_range.skipped, 0);
        assert_eq!(skip_range.from, 15);
    }
}
