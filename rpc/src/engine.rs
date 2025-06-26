use crate::jwt::{JwtMiddleware, JwtSecret};
use lumio_types::block::Block;
use lumio_types::payload::Payload;
use lumio_types::{BlockAccess, BlockAccessSender, Blocks, PayloadAccess, PayloadSender};
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

fn handle_result(
    result: Result<Result<Vec<Block>, eyre::Error>, eyre::Error>,
) -> Result<Json<Blocks>> {
    Ok(match result {
        Ok(Ok(blocks)) => Json(Blocks {
            blocks,
            error: None,
        }),
        Ok(Err(e)) => Json(Blocks {
            blocks: vec![],
            error: Some(e.to_string()),
        }),
        Err(e) => Json(Blocks {
            blocks: vec![],
            error: Some(e.to_string()),
        }),
    })
}

#[handler]
async fn get_block(Path(number): Path<u64>, state: Data<&State>) -> Result<Json<Blocks>> {
    let (tx, rx) = oneshot::channel();
    let result = state
        .block_access
        .send(BlockAccess::GetBlock {
            number,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send block access request"));

    if let Err(e) = result {
        return Ok(Json(Blocks {
            blocks: vec![],
            error: Some(e.to_string()),
        }));
    }

    let block_result = rx.await.map_err(|_| eyre::eyre!("Failed to receive block"));

    handle_result(block_result)
}

#[handler]
async fn get_latest_block(state: Data<&State>) -> Result<Json<Blocks>> {
    let (tx, rx) = oneshot::channel();
    let result = state
        .block_access
        .send(BlockAccess::GetLatestBlock { response: tx })
        .await
        .map_err(|_| eyre::eyre!("Failed to send block access request"));

    if let Err(e) = result {
        return Ok(Json(Blocks {
            blocks: vec![],
            error: Some(e.to_string()),
        }));
    }

    let block_result = rx.await.map_err(|_| eyre::eyre!("Failed to receive block"));
    handle_result(block_result)
}

#[handler]
async fn apply_payload(req: Json<Payload>, state: Data<&State>) -> Result<Json<Blocks>> {
    let (tx, rx) = oneshot::channel();
    let result = state
        .payload_access
        .send(PayloadAccess::ApplyPayload {
            payload: req.0,
            response: tx,
        })
        .await
        .map_err(|_| eyre::eyre!("Failed to send payload access request"));
    if let Err(e) = result {
        return Ok(Json(Blocks {
            blocks: vec![],
            error: Some(e.to_string()),
        }));
    }

    let block_result = rx
        .await
        .map_err(|_| eyre::eyre!("Failed to receive block ID"));
    handle_result(block_result)
}
