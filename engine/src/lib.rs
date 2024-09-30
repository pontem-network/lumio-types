use eyre::{Error, Result};
use handler::SlotHandler;
use info::L2Info;
use ledger::{Ledger, SlotArtifact, SlotAttribute};
use lumio_types::Slot;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};

pub mod handler;
pub mod info;
pub mod ledger;
pub mod sub;

pub struct EngineService<L> {
    ledger: Arc<L>,
    slot_handler: Sender<SlotAttribute>,
}

impl<L: Ledger + Send + Sync + 'static> EngineService<L> {
    pub fn new(ledger: L) -> Self {
        let ledger = Arc::new(ledger);
        let (slot_handler, sender) = SlotHandler::new(ledger.clone());
        tokio::spawn(slot_handler.run());
        Self {
            ledger,
            slot_handler: sender,
        }
    }

    pub fn info(&self) -> Result<L2Info> {
        Ok(L2Info {
            genesis_hash: self.ledger.genesis_hash()?,
            l1_confirmed_slot: self.ledger.get_committed_l1_slot()?,
            l2_version: self.ledger.get_version()?,
        })
    }

    pub fn subscribe(&self, start_from: Slot) -> Result<Receiver<SlotArtifact>, Error> {
        let (sub, rx) = sub::SlotSub::new(start_from, self.ledger.clone());
        tokio::spawn(sub.run());
        Ok(rx)
    }

    pub fn sync_channel(&self) -> Sender<SlotAttribute> {
        self.slot_handler.clone()
    }
}
