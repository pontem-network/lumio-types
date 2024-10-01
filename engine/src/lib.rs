use eyre::Result;
use handler::SlotHandler;
use info::L2Info;
use ledger::Ledger;
use lumio_types::{
    p2p::{SlotArtifact, SlotAttribute},
    Slot,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc::{Receiver, Sender};

pub mod handler;
pub mod info;
pub mod ledger;
pub mod sub;

#[derive(Clone)]
pub struct EngineService<L> {
    ledger: Arc<L>,
    sync_task_token: TaskToken,
}

impl<L: Ledger + Send + Sync + 'static> EngineService<L> {
    pub fn new(ledger: L) -> Self {
        let ledger = Arc::new(ledger);
        Self {
            ledger,
            sync_task_token: TaskToken::default(),
        }
    }

    pub fn info(&self) -> Result<L2Info> {
        Ok(L2Info {
            genesis_hash: self.ledger.genesis_hash()?,
            l1_confirmed_slot: self.ledger.get_committed_l1_slot()?,
            l2_version: self.ledger.get_version()?,
        })
    }

    pub fn subscribe(&self, start_from: Slot, sender: Sender<SlotArtifact>) {
        let sub = sub::SlotSub::new(start_from, self.ledger.clone(), sender);
        tokio::spawn(sub.run());
    }

    pub fn start_sync_task(&self, receiver: Receiver<SlotAttribute>) -> Result<()> {
        let guard = self
            .sync_task_token
            .guard()
            .ok_or_else(|| eyre::eyre!("Sync task already running"))?;
        let handler = SlotHandler::new(self.ledger.clone(), receiver, guard);
        tokio::spawn(handler.run());
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct TaskToken(Arc<AtomicBool>);

impl TaskToken {
    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn guard(&self) -> Option<TaskGuard> {
        if self
            .0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(TaskGuard(self.clone()))
        } else {
            None
        }
    }
}

pub struct TaskGuard(TaskToken);

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_token_is_active() {
        let token = TaskToken::default();
        assert!(!token.is_active());

        let guard = token.guard().unwrap();
        assert!(token.is_active());

        drop(guard);
        assert!(!token.is_active());
    }

    #[test]
    fn test_task_token_guard() {
        let token = TaskToken::default();

        // First guard should succeed
        let guard1 = token.guard();
        assert!(guard1.is_some());

        // Second guard should fail since the first one is still active
        let guard2 = token.guard();
        assert!(guard2.is_none());

        // Dropping the first guard should allow a new guard to be created
        drop(guard1);
        let guard3 = token.guard();
        assert!(guard3.is_some());
    }

    #[test]
    fn test_task_token_multiple_guards() {
        let token = TaskToken::default();

        // First guard should succeed
        let guard1 = token.guard();
        assert!(guard1.is_some());

        // Second guard should fail since the first one is still active
        let guard2 = token.guard();
        assert!(guard2.is_none());

        // Dropping the first guard should allow a new guard to be created
        drop(guard1);
        let guard3 = token.guard();
        assert!(guard3.is_some());

        // Dropping the third guard should reset the token
        drop(guard3);
        assert!(!token.is_active());
    }
}
