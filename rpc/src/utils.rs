use eyre::{Result, WrapErr};
use futures::prelude::*;

pub trait DebugableSink<I>: Sink<I, Error: std::fmt::Debug> + Unpin + 'static {}

impl<I, S> DebugableSink<I> for S
where
    S: Sink<I> + Unpin + 'static,
    S::Error: std::fmt::Debug,
{
}

pub async fn feed_receiver_to_socket(
    mut socket: poem::web::websocket::WebSocketStream,
    mut receiver: tokio::sync::mpsc::Receiver<impl serde::Serialize>,
) -> Result<()> {
    while let Some(attrs) = receiver.recv().await {
        socket
            .send(poem::web::websocket::Message::Binary(
                bincode::serialize(&attrs).unwrap(),
            ))
            .await?;
    }
    Ok(())
}

pub async fn ws_subscribe<T: serde::de::DeserializeOwned + 'static>(
    mut url: url::Url,
    path: impl IntoIterator<Item = &str>,
    queries: impl IntoIterator<Item = (&str, impl AsRef<str>)>,
    claims: String,
) -> Result<impl Stream<Item = Result<T>> + Unpin + 'static> {
    url.path_segments_mut()
        .map_err(|()| eyre::eyre!("Invalid url"))?
        .pop_if_empty()
        .extend(path);
    url.query_pairs_mut().extend_pairs(queries);

    let req = tungstenite::handshake::client::Request::builder()
        .method("GET")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {claims}"))
        .uri({
            url.set_scheme(match url.scheme().to_string().as_ref() {
                "http" => "ws",
                "https" => "wss",
                other => other,
            })
            .unwrap();
            url.to_string()
        })
        .header("Host", url.host().unwrap().to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .unwrap();
    let (stream, _) = tokio_tungstenite::connect_async(req)
        .await
        .context("Failed to connect to op-move")?;

    Ok(stream.map(|res| res.context("WS error")).and_then(|msg| {
        futures::future::ready(match msg {
            tungstenite::protocol::Message::Binary(bin) => {
                bincode::deserialize(&bin).context("Failed to deserialize message")
            }
            _ => Err(eyre::eyre!("Invalid message type")),
        })
    }))
}
