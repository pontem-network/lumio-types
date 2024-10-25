use std::sync::Arc;

use eyre::{ContextCompat, Result, WrapErr};
use futures::prelude::*;
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;
use poem::web::websocket::{self, WebSocket};
use poem::web::{Data, Path};
use poem::{Endpoint, EndpointExt, Route};

pub struct LumioConfig {
    pub op_sol: url::Url,
    pub op_move: url::Url,
}

#[derive(Clone)]
pub struct Lumio {
    since: Arc<
        std::sync::Mutex<
            Option<tokio::sync::mpsc::Receiver<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>>,
        >,
    >,
    op_sol: url::Url,
    op_move: url::Url,
}

#[derive(Clone)]
pub struct LumioState {
    since: tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>,
}

#[poem::handler]
async fn lumio_attrs_since(
    Data(state): Data<&LumioState>,
    Path(slot): Path<Slot>,
    ws: WebSocket,
) -> impl poem::web::IntoResponse {
    ws.on_upgrade({
        let handlers = state.since.clone();
        move |mut socket| async move {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
            let _ = handlers.send((slot, sender)).await;

            while let Some(attrs) = receiver.recv().await {
                socket
                    .send(websocket::Message::Binary(
                        bincode::serialize(&attrs).unwrap(),
                    ))
                    .await
                    .context("Failed to send slot attributes. Maybe socket was closed?")?;
            }

            Ok::<_, eyre::Report>(())
        }
    })
}

impl Lumio {
    pub fn new(cfg: LumioConfig) -> (Self, impl Endpoint + Send + Sync) {
        let LumioConfig { op_sol, op_move } = cfg;
        let (since_sender, since_receiver) = tokio::sync::mpsc::channel(10);
        let route = Route::new()
            .at("/attrs/since/:slot", poem::get(lumio_attrs_since))
            .with(poem::middleware::AddData::new(LumioState {
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

    pub async fn op_sol_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        todo!()
    }

    pub async fn op_move_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        todo!()
    }

    pub async fn subscribe_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        todo!()
    }

    pub async fn subscribe_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        todo!()
    }
}
