use lumio_types::{events::SlotEvents, Slot};

pub struct SkipRange {
    from: u64,
    max_slots_to_skip: u64,
    skipped: u64,
}

impl SkipRange {
    pub fn new(from: u64, max_slots_to_skip: u64) -> Self {
        Self {
            from,
            max_slots_to_skip,
            skipped: 0,
        }
    }

    pub fn try_skip<Event>(
        &mut self,
        payload: SlotEvents<Event>,
    ) -> Option<(Option<Slot>, SlotEvents<Event>)> {
        if payload.is_empty() {
            if self.skipped >= self.max_slots_to_skip {
                let from = Some(self.from);
                self.reset(payload.slot);
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
            self.reset(payload.slot);
            Some((from, payload))
        }
    }

    pub fn reset(&mut self, from: u64) {
        self.skipped = 0;
        self.from = from;
    }
}

#[cfg(test)]
mod tests {
    use lumio_types::{
        events::{lumio::LumioEvent, Transfer},
        h256::H256,
        p2p::LumioEvents,
        to::To,
    };

    use super::*;

    fn empty(slot_id: u64) -> LumioEvents {
        LumioEvents::new(slot_id, vec![])
    }

    fn with_events(slot_id: u64) -> LumioEvents {
        LumioEvents::new(
            slot_id,
            vec![To::OpSol(LumioEvent::Sol(Transfer {
                account: H256::default(),
                amount: 2,
            }))],
        )
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
        let payload = empty(11);
        assert_eq!(skip_range.try_skip(payload), None);
        assert_eq!(skip_range.skipped, 1);
    }

    #[test]
    fn test_skip_range_try_skip_non_empty_payload() {
        let mut skip_range = SkipRange::new(10, 5);
        let payload = with_events(11);
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
        let payload1 = empty(11);
        let payload2 = empty(12);
        let payload3 = empty(13);

        assert_eq!(skip_range.try_skip(payload1), None);
        assert_eq!(skip_range.skipped, 1);

        assert_eq!(skip_range.try_skip(payload2), None);
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
