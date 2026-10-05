//! Bounded relay transport. No collection policy here.
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};
use vault_core::backup::Event;
use vault_core::vault::{Error, Result};

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone, Debug, Default)]
pub struct RelayRead {
    pub events: Vec<Event>,
    pub eose: bool,
    pub saturated: bool,
    pub byte_saturated: bool,
    pub rejected: usize,
    pub auth_challenge: Option<String>,
    pub eose_more: bool,
    pub eose_auth: bool,
    pub denied: bool,
    pub disconnected: bool,
}

pub async fn connect(relay: &str) -> Result<Socket> {
    vault_core::identity::relay(relay)?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(65536))
        .max_frame_size(Some(65536));
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async_tls_with_config(
            relay,
            Some(config),
            false,
            Some(crate::identity_transport::tls_connector()?),
        ),
    )
    .await
    .map_err(|_| Error::Storage)?
    .map_err(|_| Error::Storage)?;
    Ok(socket)
}

pub async fn send(socket: &mut Socket, value: Value) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(3),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| Error::Storage)?
    .map_err(|_| Error::Storage)
}

pub async fn receive(socket: &mut Socket) -> Result<Value> {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .map_err(|_| Error::Storage)?
            .ok_or(Error::Storage)?
            .map_err(|_| Error::Storage)?;
        match message {
            Message::Text(text) => {
                return serde_json::from_str(&text).map_err(|_| Error::Malformed)
            }
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload)).await;
            }
            Message::Pong(_) => {}
            Message::Close(_) => return Err(Error::Storage),
            _ => {}
        }
    }
}

pub async fn send_auth(
    socket: &mut Socket,
    auth: &vault_core::identity::VerifiedAuth,
) -> Result<()> {
    send(socket, auth.auth_message()).await
}

pub async fn read_subscription<F>(
    socket: &mut Socket,
    sub_id: &str,
    mut on_event: F,
    max_unique: usize,
    max_bytes: usize,
) -> Result<RelayRead>
where
    F: FnMut(Event) -> Result<bool>,
{
    let mut out = RelayRead::default();
    let mut raw_bytes = 0usize;
    for _ in 0..4096 {
        let value = match receive(socket).await {
            Ok(v) => v,
            Err(Error::Storage) => {
                out.disconnected = !out.eose;
                break;
            }
            Err(e) => return Err(e),
        };
        let Some(kind) = value.get(0).and_then(|v| v.as_str()) else {
            continue;
        };
        match kind {
            "AUTH" => {
                if let Some(ch) = value.get(1).and_then(|v| v.as_str()) {
                    out.auth_challenge = Some(ch.to_string());
                }
                break;
            }
            "EOSE" if value.get(1).and_then(|v| v.as_str()) == Some(sub_id) => {
                out.eose = true;
                if value
                    .get(2)
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| a.iter().any(|h| h.as_str() == Some("more")))
                {
                    out.eose_more = true;
                }
                if value
                    .get(2)
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| a.iter().any(|h| h.as_str() == Some("auth")))
                {
                    out.eose_auth = true;
                }
                let _ = send(socket, json!(["CLOSE", sub_id])).await;
                break;
            }
            "EVENT" if value.get(1).and_then(|v| v.as_str()) == Some(sub_id) => {
                if value.as_array().map(Vec::len) != Some(3) {
                    out.rejected += 1;
                    continue;
                }
                let event: Event = match serde_json::from_value(value[2].clone()) {
                    Ok(e) => e,
                    Err(_) => {
                        out.rejected += 1;
                        continue;
                    }
                };
                let size = serde_json::to_vec(&event)
                    .map_err(|_| Error::Malformed)?
                    .len();
                if out.events.len() >= max_unique {
                    out.saturated = true;
                    break;
                }
                if raw_bytes.saturating_add(size) > max_bytes {
                    out.byte_saturated = true;
                    break;
                }
                match on_event(event.clone()) {
                    Ok(true) => {
                        raw_bytes = raw_bytes.saturating_add(size);
                        out.events.push(event);
                    }
                    Ok(false) => out.rejected += 1,
                    Err(_) => out.rejected += 1,
                }
            }
            "CLOSED" if value.get(1).and_then(|v| v.as_str()) == Some(sub_id) => {
                let msg = value.get(2).and_then(|v| v.as_str()).unwrap_or("");
                if msg.starts_with("auth-required:") {
                    out.auth_challenge.get_or_insert_with(|| {
                        msg.strip_prefix("auth-required:")
                            .unwrap_or(msg)
                            .trim()
                            .to_string()
                    });
                } else if msg.starts_with("restricted:") {
                    out.denied = true;
                }
                break;
            }
            _ => {}
        }
    }
    Ok(out)
}
