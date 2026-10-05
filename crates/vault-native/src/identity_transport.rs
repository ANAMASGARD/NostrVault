//! Native socket host. The portable engine owns all protocol decisions.
use crate::vault::Runtime;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use vault_core::{
    identity::{Binding, WIRE_LIMIT},
    vault::{Error, Result},
};
fn tls_connector() -> Result<tokio_tungstenite::Connector> {
    // Use an explicit provider so portable/headless hosts do not depend on a
    // different consumer enabling rustls defaults. Certificate validation stays mandatory.
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| Error::Unsupported)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(tokio_tungstenite::Connector::Rustls(Arc::new(config)))
}
pub async fn receive(
    runtime: Arc<Runtime>,
    binding: Binding,
    id: String,
) -> Result<serde_json::Value> {
    let effect = runtime.identity_effect(&binding, &id)?;
    if effect.adapter != vault_core::identity::Adapter::Remote {
        return Err(Error::Unsupported);
    }
    // Endpoints originate only from the approved engine effect, never this command's arguments.
    for relay in effect.relays {
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(WIRE_LIMIT))
            .max_frame_size(Some(WIRE_LIMIT));
        let connected = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio_tungstenite::connect_async_tls_with_config(
                &relay,
                Some(config),
                false,
                Some(tls_connector()?),
            ),
        )
        .await;
        let Ok(Ok((mut socket, _))) = connected else {
            continue;
        };
        runtime.identity_effect(&binding, &id)?;
        for message in &effect.messages {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    message.to_string().into(),
                ))
                .await
                .map_err(|_| Error::Storage)?;
        }
        loop {
            if runtime.identity_effect(&binding, &id).is_err() {
                let _ = socket.close(None).await;
                return Err(Error::Cancelled);
            }
            let read = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next()).await;
            let message = match read {
                Err(_) => continue,
                Ok(Some(Ok(m))) => m,
                _ => break,
            };
            if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(|_| Error::Malformed)?;
                if value[0] == "EVENT"
                    && value[1] == id
                    && value.as_array().is_some_and(|a| a.len() == 3)
                {
                    let _ = socket.close(None).await;
                    return Ok(value[2].clone());
                }
            }
        }
    }
    Err(Error::Storage)
}

#[cfg(test)]
mod tests {
    #[test]
    fn standalone_native_host_has_an_explicit_tls_provider_and_roots() {
        assert!(!webpki_roots::TLS_SERVER_ROOTS.is_empty());
        assert!(super::tls_connector().is_ok());
    }
}
