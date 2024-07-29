use std::{
    fmt::{self, Display, Formatter},
    time::Duration,
};

use anyhow::{anyhow, Error};
use log::{debug, warn};
use lumio_rpc::AttributesArtifact;
use lumio_types::Slot;
use tokio::sync::oneshot::{self, error::TryRecvError};

use crate::ledger::Ledger;

use super::{collector::ArtifactCollector, loader::SlotLoader};

pub fn make_task(id: TaskId) -> (TaskHandler, Task) {
    let (sender, receiver) = oneshot::channel();
    let handler = TaskHandler {
        id,
        rusult: receiver,
        submited: false,
    };
    let task = Task {
        id,
        collector: ArtifactCollector::new(id.parent_id, id.payload_size),
        result_channel: Some(sender),
        retry: 8,
        started_at: std::time::Instant::now(),
    };
    (handler, task)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId {
    pub parent_id: Slot,
    pub payload_size: u32,
}

type TaskResult = Result<AttributesArtifact, Error>;

pub struct TaskHandler {
    id: TaskId,
    submited: bool,
    rusult: oneshot::Receiver<TaskResult>,
}

impl TaskHandler {
    pub async fn try_get_result(&mut self) -> Result<Option<TaskResult>, Error> {
        match self.rusult.try_recv() {
            Ok(res) => Ok(Some(res)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => {
                Err(anyhow!("TaskHandler with id {:?} was closed", self.id))
            }
        }
    }

    pub fn is_submited(&self) -> bool {
        self.submited
    }

    pub fn submited(&mut self) {
        self.submited = true;
    }
}

pub struct Task {
    id: TaskId,
    collector: ArtifactCollector,
    result_channel: Option<oneshot::Sender<TaskResult>>,
    retry: u8,
    started_at: std::time::Instant,
}

impl Task {
    pub fn is_done(&self) -> bool {
        self.result_channel.is_none()
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn statistic(&self) -> Statictic {
        Statictic {
            duration: self.started_at.elapsed(),
            progress: self.collector.size() as f64 / self.collector.max_payload_size() as f64
                * 100.0,
            size: self.collector.size(),
            max_size: self.collector.max_payload_size(),
            slots: self.collector.slots_count(),
        }
    }

    pub async fn poll<L: Ledger>(&mut self, loader: &mut SlotLoader<L>) {
        if self.is_done() {
            return;
        }

        match self.try_progress(loader).await {
            Ok(Some(artifact)) => {
                debug!(
                    "Task:{:?} is done. Duration: {:?}",
                    self.id,
                    self.started_at.elapsed()
                );
                if self
                    .result_channel
                    .take()
                    .unwrap()
                    .send(Ok(artifact))
                    .is_err()
                {
                    warn!("Failed to send result for task:{:?}.", self.id);
                }
            }
            Ok(None) => {}
            Err(err) => {
                if self.retry > 0 {
                    warn!(
                        "Failed to process payload task:{:?}. Error:{}.",
                        self.id, err
                    );
                    self.retry -= 1;
                } else {
                    warn!(
                        "Failed to process payload task:{:?}. Error:{}. Duration: {:?}",
                        self.id,
                        err,
                        self.started_at.elapsed()
                    );
                    if self.result_channel.take().unwrap().send(Err(err)).is_err() {
                        warn!("Failed to send result for task:{:?}.", self.id);
                    }
                }
            }
        }
    }

    async fn try_progress<L: Ledger>(
        &mut self,
        loader: &mut SlotLoader<L>,
    ) -> Result<Option<AttributesArtifact>, Error> {
        loop {
            let next_slot = self.collector.next_slot();
            if let Some((slot, events)) = loader.get_slot(next_slot).await? {
                if !self.collector.try_add(slot, events) {
                    return Ok(Some(self.collector.collect()));
                }
            } else {
                break;
            }
        }
        Ok(None)
    }
}

pub struct Statictic {
    duration: Duration,
    progress: f64,
    size: usize,
    max_size: u32,
    slots: usize,
}

impl Display for Statictic {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Duration: {:?}, Progress: {:.2}%, Size: {}, Max Size: {} kib, Slots: {} kib",
            self.duration,
            self.progress,
            self.size / 1024,
            self.max_size / 1024,
            self.slots,
        )
    }
}
