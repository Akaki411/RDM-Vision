use std::net::SocketAddr;

use futures::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use crate::config::WebSocketConfig;
use crate::error::Result;

// Емкость канала
const CHANNEL_CAPACITY: usize = 256;

// Интерфейс сообщения
#[derive(Debug, Clone, Serialize)]
pub struct CodeMessage
{
    pub camera_id: String,
    pub code: String,
    pub restored: bool,
    pub time_ms: u64
}

pub struct CodeServer
{
    tx: broadcast::Sender<String>
}

impl CodeServer
{
    pub async fn start(cfg: &WebSocketConfig) -> Result<Self>
    {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(%addr, "websocket server listening");

        let accept_tx = tx.clone();
        tokio::spawn(async move
        {
            loop
            {
                match listener.accept().await
                {
                    Ok((stream, peer)) =>
                    {
                        tokio::spawn(serve_client(stream, peer, accept_tx.subscribe()));
                    }
                    Err(err) => tracing::warn!(error = %err, "websocket accept failed")
                }
            }
        });

        return Ok(Self { tx });
    }

    // Разослать код всем клиентам
    pub fn broadcast(&self, message: &CodeMessage)
    {
        match serde_json::to_string(message)
        {
            Ok(json) =>
            {
                let _ = self.tx.send(json);
            }
            Err(err) => tracing::error!(error = %err, "failed to serialize code message")
        }
    }
}

// Обслуживание одного клиента
async fn serve_client(stream: TcpStream, peer: SocketAddr, mut rx: broadcast::Receiver<String>)
{
    let ws = match tokio_tungstenite::accept_async(stream).await
    {
        Ok(ws) => ws,
        Err(err) =>
        {
            tracing::warn!(%peer, error = %err, "websocket handshake failed");
            return;
        }
    };

    tracing::info!(%peer, "websocket client connected");
    let (mut sink, mut source) = ws.split();

    loop
    {
        tokio::select!
        {
            outgoing = rx.recv() =>
            {
                match outgoing
                {
                    Ok(json) =>
                    {
                        if sink.send(Message::Text(json.into())).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) =>
                    {
                        tracing::warn!(%peer, skipped, "websocket client lagged, dropped codes");
                    }
                    Err(broadcast::error::RecvError::Closed) => break
                }
            }
            incoming = source.next() =>
            {
                match incoming
                {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::info!(%peer, "websocket client disconnected");
}
