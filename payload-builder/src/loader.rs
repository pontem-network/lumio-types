use std::collections::BTreeMap;

use anyhow::Error;
use lumio_types::Slot;
// use sol_l2_withdrawal::WithdrawalInstruction;
// use solana_rpc::rpc::JsonRpcRequestProcessor;
// use solana_sdk::hash::Hash;
// use solana_transaction_status::TransactionWithStatusMeta;

use super::SlotArtifact;
use crate::ledger::Ledger;

pub struct SlotLoader<L> {
    ledger: L,
    permits: u32,
    cache: SlotCache,
}

impl<L: Ledger> SlotLoader<L> {
    pub fn new(ledger: L, max_cache_size: u32) -> Self {
        Self {
            ledger,
            permits: 0,
            cache: SlotCache::new(max_cache_size),
        }
    }

    pub async fn warmup(&mut self) -> Result<(), Error> {
        let current_slot = self.ledger.get_current_slot()?;
        if self.cache.contains(current_slot) {
            return Ok(());
        }

        let last_slot_in_cache = self.cache.last_index();
        let (start, end) = if let Some(last_slot_in_cache) = last_slot_in_cache {
            (last_slot_in_cache, current_slot)
        } else {
            (current_slot, current_slot)
        };

        for slot in start..=end {
            self.permits = self.permits.saturating_sub(1);
            if self.permits == 0 {
                break;
            }

            if let Some(slot) = self.get_slot(slot).await? {
                self.cache.insert(slot);
            }
        }

        Ok(())
    }

    pub fn set_permits(&mut self, permits: u32) {
        self.permits = permits;
    }

    pub fn add_permits(&mut self, permits: u32) {
        self.permits += permits;
    }

    pub async fn get_slot(&mut self, next_slot: Slot) -> Result<Option<SlotArtifact>, Error> {
        if let Some(from_cache) = self.cache.get(next_slot) {
            return Ok(Some(from_cache.clone()));
        }

        if self.permits == 0 {
            return Ok(None);
        }
        self.permits -= 1;

        if let Some(slot) = self.ledger.get_slot(next_slot).await? {
            self.cache.insert(slot.clone());
            Ok(Some(slot))
        } else {
            Ok(None)
        }
    }
}

struct SlotCache {
    max_size: u32,
    slots: BTreeMap<Slot, SlotArtifact>,
}

impl SlotCache {
    fn new(max_cache_size: u32) -> SlotCache {
        Self {
            max_size: max_cache_size,
            slots: BTreeMap::new(),
        }
    }

    fn contains(&self, slot: Slot) -> bool {
        self.get(slot).is_some()
    }

    fn get(&self, slot: Slot) -> Option<&SlotArtifact> {
        self.slots.get(&slot)
    }

    fn last_index(&self) -> Option<Slot> {
        self.slots.last_key_value().map(|(k, _)| *k)
    }

    fn insert(&mut self, slot_payload: SlotArtifact) {
        if self.slots.len() as u32 >= self.max_size {
            if let Some(e) = self.slots.first_entry() {
                e.remove();
            }
        }
        self.slots.insert(slot_payload.0.slot, slot_payload);
    }
}
