use std::sync::Arc;

use eyre::{ContextCompat, Result, WrapErr};
use futures::prelude::*;
use lumio_types::events::l2::EngineActions;
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;
use poem::http::StatusCode;
use poem::web::websocket::WebSocket;
use poem::web::{Data, Query};
use poem::{Endpoint, EndpointExt, Route};
use tokio::sync::mpsc;

pub struct Config {
    pub lumio: url::Url,
    pub other_engine: url::Url,
}

#[derive(Clone)]
pub struct Engine {
    events:
        Arc<std::sync::Mutex<Option<mpsc::Receiver<(Slot, mpsc::Sender<SlotPayloadWithEvents>)>>>>,
    engine: Arc<std::sync::Mutex<Option<mpsc::Receiver<(Slot, mpsc::Sender<EngineActions>)>>>>,
    finalize: Arc<std::sync::Mutex<Option<mpsc::Receiver<(Slot, PayloadStatus)>>>>,
    lumio: url::Url,
    other_engine: url::Url,
}

#[derive(Clone)]
struct State {
    events: mpsc::Sender<(Slot, mpsc::Sender<SlotPayloadWithEvents>)>,
    engine: mpsc::Sender<(Slot, mpsc::Sender<EngineActions>)>,
    finalize: mpsc::Sender<(Slot, PayloadStatus)>,
}

#[poem::handler]
async fn events_since(
    Data(state): Data<&State>,
    Query(slot): Query<Slot>,
    ws: WebSocket,
) -> impl poem::web::IntoResponse {
    ws.on_upgrade({
        let handlers = state.events.clone();
        move |mut socket| async move {
            let (sender, mut receiver) = mpsc::channel(10);
            let _ = handlers.send((slot, sender)).await;
            crate::utils::feed_receiver_to_socket(socket, receiver)
                .await
                .context("Failed to send slot attributes. Maybe socket was closed?")
        }
    })
}

#[poem::handler]
async fn engine_since(
    Data(state): Data<&State>,
    Query(slot): Query<Slot>,
    ws: WebSocket,
) -> impl poem::web::IntoResponse {
    ws.on_upgrade({
        let handlers = state.engine.clone();
        move |mut socket| async move {
            let (sender, mut receiver) = mpsc::channel(10);
            let _ = handlers.send((slot, sender)).await;
            crate::utils::feed_receiver_to_socket(socket, receiver)
                .await
                .context("Failed to send slot attributes. Maybe socket was closed?")
        }
    })
}

#[derive(serde::Deserialize)]
pub(crate) struct Finalize {
    pub slot: Slot,
    pub status: PayloadStatus,
}

#[poem::handler]
async fn finalize(
    Data(state): Data<&State>,
    Query(Finalize { slot, status }): Query<Finalize>,
) -> impl poem::web::IntoResponse {
    if state.finalize.send((slot, status)).await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send request to finalize slot",
        );
    }

    (StatusCode::OK, "")
}

impl Engine {
    pub fn new(cfg: Config) -> (Self, impl Endpoint + Send + Sync) {
        let Config {
            lumio,
            other_engine,
        } = cfg;
        let (events_sender, events_receiver) = tokio::sync::mpsc::channel(10);
        let (engine_sender, engine_receiver) = tokio::sync::mpsc::channel(10);
        let (finalize_sender, finalize_receiver) = tokio::sync::mpsc::channel(10);
        let route = Route::new()
            .at("/events", poem::get(events_since))
            .at("/engine", poem::get(engine_since))
            .at("/finalize", poem::get(finalize))
            .with(poem::middleware::AddData::new(State {
                engine: engine_sender,
                events: events_sender,
                finalize: finalize_sender,
            }));
        let me = Self {
            engine: Arc::new(std::sync::Mutex::new(Some(engine_receiver))),
            events: Arc::new(std::sync::Mutex::new(Some(events_receiver))),
            finalize: Arc::new(std::sync::Mutex::new(Some(finalize_receiver))),
            lumio,
            other_engine,
        };
        (me, route)
    }

    pub async fn handle_events_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotPayloadWithEvents> + Unpin + 'static)> + Unpin + 'static,
    > {
        let receiver = self
            .events
            .lock()
            .unwrap()
            .take()
            .context("Only 1 handler is supported")?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_engine_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<EngineActions> + Unpin + 'static)> + Unpin + 'static,
    > {
        let receiver = self
            .engine
            .lock()
            .unwrap()
            .take()
            .context("Only 1 handler is supported")?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_finalize(
        &self,
    ) -> Result<impl Stream<Item = (Slot, PayloadStatus)> + Unpin + 'static> {
        self.finalize
            .lock()
            .unwrap()
            .take()
            .map(tokio_stream::wrappers::ReceiverStream::new)
            .context("Only 1 handler is supported")
    }

    pub async fn subscribe_op_move_engine_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotAttribute> + Unpin + 'static)> + Unpin + 'static,
    > {
        todo!()
    }

    pub async fn subscribe_lumio_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        todo!()
    }
}
