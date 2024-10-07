use eyre::Result;
use lumio_engine::ledger::Ledger;
use lumio_types::{
    p2p::{SlotAttribute, SlotPayload, SlotPayloadWithEvents},
    Hash, Slot,
};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct FakeLedger {
    current_slot: AtomicU64,
    committed_l1_slot: AtomicU64,
}

impl Ledger for FakeLedger {
    fn get_committed_l1_slot(&self) -> Result<lumio_types::Slot> {
        Ok(self.committed_l1_slot.load(Ordering::SeqCst))
    }

    fn genesis_hash(&self) -> Result<lumio_types::Hash> {
        Ok(Hash::default())
    }

    fn get_slot(
        &self,
        slot_id: lumio_types::Slot,
    ) -> impl std::future::Future<Output = Result<Option<SlotPayloadWithEvents>>> + Send {
        let res = self.current_slot.compare_exchange(
            slot_id,
            slot_id + 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        let result = match res {
            Ok(_) => {
                let payload = SlotPayload {
                    slot: slot_id,
                    previous_blockhash: Hash::default(),
                    blockhash: Hash::default(),
                    block_time: None,
                    block_height: None,
                    txs: vec![],
                    bank_hash: Hash::default(),
                };
                let artifact = SlotPayloadWithEvents {
                    payload,
                    events: vec![],
                };
                Some(artifact)
            }
            Err(_) => None,
        };
        async move { Ok(result) }
    }

    fn apply_slot(
        &self,
        _skip_from: Option<Slot>,
        _slot: SlotAttribute,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}
