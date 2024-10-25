use eyre::Result;
use futures::prelude::*;

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
