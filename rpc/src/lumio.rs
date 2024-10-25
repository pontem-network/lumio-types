use std::sync::Arc;

use eyre::{ContextCompat, Result, WrapErr};
use futures::prelude::*;
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;
use poem::web::websocket::WebSocket;
use poem::web::{Data, Query};
use poem::{Endpoint, EndpointExt, Route};
use tokio::sync::mpsc;

pub struct Config {
    pub op_sol: url::Url,
    pub op_move: url::Url,
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct Lumio {
    since: Arc<std::sync::Mutex<Option<mpsc::Receiver<(Slot, mpsc::Sender<SlotAttribute>)>>>>,
    op_sol: url::Url,
    op_move: url::Url,
}

#[derive(Clone)]
struct State {
    since: mpsc::Sender<(Slot, mpsc::Sender<SlotAttribute>)>,
}

#[poem::handler]
async fn attrs_since(
    Data(state): Data<&State>,
    Query(slot): Query<Slot>,
    ws: WebSocket,
) -> impl poem::web::IntoResponse {
    ws.on_upgrade({
        let handlers = state.since.clone();
        move |mut socket| async move {
            let (sender, mut receiver) = mpsc::channel(10);
            let _ = handlers.send((slot, sender)).await;
            crate::utils::feed_receiver_to_socket(socket, receiver)
                .await
                .context("Failed to send slot attributes. Maybe socket was closed?")
        }
    })
}

impl Lumio {
    pub fn new(cfg: Config) -> (Self, impl Endpoint) {
        let Config { op_sol, op_move } = cfg;
        let (since_sender, since_receiver) = mpsc::channel(10);
        let route =
            Route::new()
                .at("/attrs", poem::get(attrs_since))
                .with(poem::middleware::AddData::new(State {
                    since: since_sender,
                }));
        let me = Self {
            since: Arc::new(std::sync::Mutex::new(Some(since_receiver))),
            op_sol,
            op_move,
        };
        (me, route)
    }

    pub async fn handle_lumio_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotAttribute> + Unpin + 'static)> + Unpin + 'static,
    > {
        let receiver = self
            .since
            .lock()
            .unwrap()
            .take()
            .context("Only 1 handler is supported")?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    async fn finalize(
        &self,
        mut url: url::Url,
        slot: Slot,
        status: PayloadStatus,
    ) -> reqwest::Result<()> {
        url.set_path("/finalize");
        reqwest::Client::new()
            .get(url)
            .query(&crate::engine::Finalize { slot, status })
            .send()
            .await
            .map(drop)
    }

    pub async fn op_sol_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        self.finalize(self.op_sol.clone(), slot, status)
            .await
            .context("Failed to finalize op-sol")
    }

    pub async fn op_move_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        self.finalize(self.op_sol.clone(), slot, status)
            .await
            .context("Failed to finalize op-move")
    }

    pub async fn subscribe_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = Result<SlotPayloadWithEvents>> + Unpin + 'static> {
        let mut url = self.op_move.clone();
        url.set_path("/events");
        url.set_query(Some(&since.to_string()));
        crate::utils::ws_subscribe(url)
            .await
            .context("Failed to subscribe to op-move events")
    }

    pub async fn subscribe_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = Result<SlotPayloadWithEvents>> + Unpin + 'static> {
        let mut url = self.op_sol.clone();
        url.set_path("/events");
        url.set_query(Some(&since.to_string()));
        crate::utils::ws_subscribe(url)
            .await
            .context("Failed to subscribe to op-sol events")
    }
}
