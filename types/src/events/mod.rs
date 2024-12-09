use serde::{Deserialize, Serialize};

use crate::{to::To, Address, Slot};

pub mod engine;
pub mod lumio;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    pub account: Address,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotEvents<Event> {
    pub slot: Slot,
    pub events: Vec<To<Event>>,
}

impl<Event> SlotEvents<Event> {
    pub fn new(slot: Slot, events: Vec<To<Event>>) -> Self {
        Self { slot, events }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
