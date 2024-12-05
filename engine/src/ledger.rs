use eyre::Error;
use lumio_types::events::engine::EngineEvent;
use lumio_types::events::lumio::LumioEvent;
use lumio_types::events::SlotEvents;
use lumio_types::p2p::{EngineEvents, LumioEvents, SlotPayloadWithEvents};
use lumio_types::{Hash, Slot};

pub trait Ledger {
    /// Get the committed lumio slot.
    fn get_committed_lumio_slot(&self) -> Result<Slot, Error>;
    /// Get the committed engine slot.
    fn get_committed_engine_slot(&self) -> Result<Slot, Error>;
    /// genesis hash
    fn genesis_hash(&self) -> Result<Hash, Error>;
    /// Get the slot artifact by slot id. If the slot with slot_id skipped, return next slot.
    fn get_payload(&self, slot_id: Slot) -> Result<SlotPayloadWithEvents, Error>;
    /// Get the events by slot id.
    fn get_events(&self, slot_id: Slot) -> Result<EngineEvents, Error>;
    /// Apply events from lumio.
    fn apply_lumio_event(&self, skip_from: Option<Slot>, slot: LumioEvents) -> Result<(), Error>;
    /// Apply events from engine.
    fn apply_engine_event(
        &self,
        skip_from: Option<Slot>,
        actions: EngineEvents,
    ) -> Result<(), Error>;
}

pub trait ApplyAbstraction<Event> {
    fn get_committed_slot(&self) -> Result<Slot, Error>;
    fn apply_events(&self, skip_from: Option<Slot>, slot: SlotEvents<Event>) -> Result<(), Error>;
}

pub struct LedgerAbs<L, Event> {
    ledger: L,
    _event: std::marker::PhantomData<Event>,
}

impl<L: Ledger + Send + Sync + 'static, Event> LedgerAbs<L, Event> {
    pub fn new(ledger: L) -> LedgerAbs<L, Event> {
        LedgerAbs {
            ledger,
            _event: std::marker::PhantomData,
        }
    }
}

impl<L> ApplyAbstraction<LumioEvent> for LedgerAbs<L, LumioEvent>
where
    L: Ledger + Send + Sync + 'static,
{
    fn get_committed_slot(&self) -> Result<Slot, Error> {
        self.ledger.get_committed_lumio_slot()
    }

    fn apply_events(
        &self,
        skip_from: Option<Slot>,
        slot: SlotEvents<LumioEvent>,
    ) -> Result<(), Error> {
        self.ledger.apply_lumio_event(skip_from, slot)
    }
}

impl<L> ApplyAbstraction<EngineEvent> for LedgerAbs<L, EngineEvent>
where
    L: Ledger + Send + Sync + 'static,
{
    fn get_committed_slot(&self) -> Result<Slot, Error> {
        self.ledger.get_committed_engine_slot()
    }

    fn apply_events(
        &self,
        skip_from: Option<Slot>,
        slot: SlotEvents<EngineEvent>,
    ) -> Result<(), Error> {
        self.ledger.apply_engine_event(skip_from, slot)
    }
}

pub trait PayloadAbstraction<Payload> {
    fn get_payload(&self, slot_id: Slot) -> Result<Payload, Error>;
}

impl<L: Ledger + Send + Sync + 'static> PayloadAbstraction<SlotPayloadWithEvents>
    for LedgerAbs<L, SlotPayloadWithEvents>
{
    fn get_payload(&self, slot_id: Slot) -> Result<SlotPayloadWithEvents, Error> {
        self.ledger.get_payload(slot_id)
    }
}

impl<L: Ledger + Send + Sync + 'static> PayloadAbstraction<EngineEvents>
    for LedgerAbs<L, EngineEvents>
{
    fn get_payload(&self, slot_id: Slot) -> Result<EngineEvents, Error> {
        self.ledger.get_events(slot_id)
    }
}
