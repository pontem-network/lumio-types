mod collector;
mod config;
mod ledger;
mod loader;
mod service;
mod task;

pub use config::PayloadBuilderConfig;
pub use ledger::Ledger;

use std::collections::HashMap;

use anyhow::Error;
use lumio_rpc::{AttributesArtifact, SlotEvents};
use lumio_types::{events::l2::L2Event, payload::SlotPayload};
use service::PayloadService;
use task::{Task, TaskHandler, TaskId};
use tokio::sync::{mpsc::Sender, Mutex};

pub type SlotArtifact = (SlotPayload, SlotEvents<L2Event>);

pub struct PayloadBuilder {
    handlers: Mutex<HashMap<TaskId, TaskHandler>>,
    task_sender: Sender<Task>,
}

impl PayloadBuilder {
    pub async fn spawn<L: Ledger + Send + 'static>(
        ledger: L,
        cfg: &PayloadBuilderConfig,
    ) -> PayloadBuilder {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);

        let builder = PayloadBuilder {
            handlers: Mutex::new(HashMap::new()),
            task_sender: sender,
        };
        let slots_per_tick = cfg.max_slots_per_builder_tick.get();
        let max_cache_size = cfg.max_cache_size.get();

        let srv = PayloadService::new(ledger, receiver, slots_per_tick, max_cache_size);
        tokio::spawn(async move {
            srv.run().await;
        });

        builder
    }

    pub async fn build(
        &self,
        parent_payload: u64,
        max_payload_size: u32,
    ) -> Result<Option<AttributesArtifact>, Error> {
        let id = TaskId {
            parent_id: parent_payload,
            payload_size: max_payload_size,
        };

        let mut handlers = self.handlers.lock().await;
        if let Some(handler) = handlers.get_mut(&id) {
            return match handler.try_get_result().await {
                Ok(Some(result)) => {
                    handlers.remove(&id);
                    println!("Got result");
                    Ok(Some(result?))
                }
                Ok(None) => Ok(None),
                Err(err) => {
                    handlers.remove(&id);
                    log::error!("Failed to get result: {:?}", err);
                    Err(err)
                }
            };
        }

        let task_entry = handlers.entry(id);
        let (handler, task) = task::make_task(id);
        let handler = task_entry.or_insert(handler);
        if !handler.is_submited() {
            self.task_sender.send(task).await?;
            handler.submited();
        }
        Ok(None)
    }
}
