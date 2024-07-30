use std::time::Duration;

use log::{debug, error};
use tokio::{select, sync::mpsc::Receiver};

use crate::ledger::Ledger;

use super::{loader::SlotLoader, task::Task};

pub struct PayloadService<L> {
    loader: SlotLoader<L>,
    tasks: Vec<Task>,
    listener: Receiver<Task>,
    slots_per_tick: u32,
}

impl<L: Ledger> PayloadService<L> {
    pub fn new(
        ledger: L,
        receiver: Receiver<Task>,
        slots_per_tick: u32,
        max_cache_size: u32,
    ) -> Self {
        Self {
            loader: SlotLoader::new(ledger, max_cache_size),
            tasks: vec![],
            listener: receiver,
            slots_per_tick,
        }
    }

    pub async fn run(mut self) {
        let mut slot_interval = tokio::time::interval(Duration::from_millis(400));
        let mut statistic_interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            self.loader.set_permits(0);
            select! {
                _ = slot_interval.tick() => {
                    self.process_tick().await;
                }
                _ = statistic_interval.tick() => {
                    self.print_statisitc();
                }
                new_task = self.listener.recv() => {
                    if let Some(new_task) = new_task {
                        self.process_request(new_task).await;
                    }
                }
            }
        }
    }

    async fn process_tick(&mut self) {
        if !self.tasks.is_empty() {
            self.loader.set_permits(0);

            let tiks_per_task = self.slots_per_tick / self.tasks.len() as u32 + 1;
            for task in &mut self.tasks {
                self.loader.add_permits(tiks_per_task);
                task.poll(&mut self.loader).await;
            }
        } else {
            self.loader.add_permits(self.slots_per_tick);
        }

        if let Err(err) = self.loader.warmup().await {
            error!("Failed to warmup: {:?}", err);
        }

        self.tasks.retain(|t| !t.is_done());
    }

    fn print_statisitc(&self) {
        if self.tasks.is_empty() {
            return;
        }
        debug!("Tasks in progress:");
        for task in &self.tasks {
            debug!(
                "   Task: {:?} is in progress.: {}",
                task.id().parent_id,
                task.statistic()
            );
        }
    }

    async fn process_request(&mut self, task: Task) {
        debug!("New task: {:?}", task.id());
        self.tasks.push(task)
    }
}
