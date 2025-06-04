use crate::jwt::{JwtMiddleware, JwtSecret};
use lumio_types::payload::Payload;
use lumio_types::{block::Block, Hash};
use lumio_types::{BlockAccess, BlockAccessSender, PayloadAccess, PayloadSender};
use poem::listener::TcpListener;
use poem::middleware::AddData;
use poem::web::{Data, Json};
use poem::{get, handler, web::Path, Route};
use poem::{post, EndpointExt as _, Result, Server};
use std::net::SocketAddr;
use tokio::sync::oneshot;

#[derive(Clone)]
struct State {
    block_access: BlockAccessSender,
    payload_access: PayloadSender,
}

pub async fn spawn(
    jwt: JwtSecret,
    addr: SocketAddr,
    block_access: BlockAccessSender,
    payload_access: PayloadSender,
) {
    let state = State {
        block_access,
        payload_access,
    };

    let app = Route::new()
        .at("/block/:id", get(get_block))
        .at("/payload", post(apply_payload))
        .at("/block/next/:id", get(get_next_block))
        .with(JwtMiddleware(jwt))
        .with(AddData::new(state));

    Server::new(TcpListener::bind(addr)).run(app).await.unwrap();
}

#[handler]
async fn get_block(Path(block_id): Path<Hash>, state: Data<&State>) -> Result<Json<Block>> {
    let (tx, rx) = oneshot::channel();
    state
        .block_access
        .send(BlockAccess::GetBlock {
            payload_id: block_id,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send block access request"))?;

    let block = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive block"))??;
    Ok(Json(block))
}

#[handler]
async fn apply_payload(req: Json<Payload>, state: Data<&State>) -> Result<Json<Block>> {
    let (tx, rx) = oneshot::channel();
    state
        .payload_access
        .send(PayloadAccess::ApplyPayload {
            payload: req.0,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send payload access request"))?;

    let block_id = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive block ID"))??;
    Ok(Json(block_id))
}

#[handler]
async fn get_next_block(Path(block_id): Path<Hash>, state: Data<&State>) -> Result<Json<Hash>> {
    let (tx, rx) = oneshot::channel();
    state
        .block_access
        .send(BlockAccess::GetNextBlock {
            id: block_id,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send next block access request"))?;

    let next_block_id = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive next block ID"))??;
    Ok(Json(next_block_id))
}
