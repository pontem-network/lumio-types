use crate::jwt::{JwtMiddleware, JwtSecret};
use lumio_types::block::Block;
use lumio_types::payload::Payload;
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
        .at("/block/:number", get(get_block))
        .at("/payload", post(apply_payload))
        .at("/block/latest", get(get_latest_block))
        .with(JwtMiddleware(jwt))
        .with(AddData::new(state));

    Server::new(TcpListener::bind(addr)).run(app).await.unwrap();
}

#[handler]
async fn get_block(Path(number): Path<u64>, state: Data<&State>) -> Result<Json<Block>> {
    let (tx, rx) = oneshot::channel();
    state
        .block_access
        .send(BlockAccess::GetBlock {
            number,
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
async fn get_latest_block(state: Data<&State>) -> Result<Json<Block>> {
    let (tx, rx) = oneshot::channel();
    state
        .block_access
        .send(BlockAccess::GetLatestBlock { response: tx })
        .await
        .map_err(|_| eyre::eyre!("Failed to send block access request"))?;

    let block = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive block"))??;
    Ok(Json(block))
}

#[handler]
async fn apply_payload(req: Json<Payload>, state: Data<&State>) -> Result<Json<Vec<Block>>> {
    let (tx, rx) = oneshot::channel();
    state
        .payload_access
        .send(PayloadAccess::ApplyPayload {
            payload: req.0,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send payload access request"))?;

    let block = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive block ID"))??;
    Ok(Json(block))
}
