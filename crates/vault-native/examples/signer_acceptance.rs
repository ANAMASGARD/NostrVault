//! Explicit real-signer acceptance client. All account material is disposable.
use futures_util::{SinkExt, StreamExt};
use std::{path::PathBuf, sync::Arc};
use vault_core::{
    identity::{Action, Adapter, Binding, Grants, Output, Request as IdentityRequest},
    vault::{Operation, Request},
};
use vault_native::{identity_transport, vault::Runtime};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or("directory")?);
    let relay = args.next().ok_or("relay")?;
    let pairing = args.next().ok_or("synthetic pairing URL")?;
    let runtime = Arc::new(Runtime::new(directory)?);
    let mut request = Request {
        version: 1,
        request_id: "acceptance-vault".into(),
        token: String::new(),
        generation: 0,
        vault_id: None,
        operation: Operation::Status,
    };
    let status = runtime.execute(&request)?;
    request.token = status.token;
    request.generation = status.generation;
    request.vault_id = status.vault_id;
    request.operation = Operation::Create {
        password: "disposable signer acceptance".into(),
        confirmation: "disposable signer acceptance".into(),
    };
    let status = runtime.execute(&request)?;
    let mut binding = Binding {
        vault_id: status.vault_id.ok_or("vault ID")?,
        token: status.token,
        generation: status.generation,
        account: None,
        signer_generation: 0,
        consent_revision: 0,
    };
    let mut sequence = 0;
    let mut invoke = |action: Action| -> Result<Output, vault_core::vault::Error> {
        sequence += 1;
        let output = runtime.identity_execute(IdentityRequest {
            request_id: format!("acceptance-{sequence}"),
            binding: binding.clone(),
            action,
        })?;
        binding = output.binding.clone();
        Ok(output)
    };
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut output = invoke(Action::Connect {
        adapter: Adapter::Remote,
        remember: true,
        pairing: Some(pairing),
        relays: vec![relay.clone()],
        package: None,
    })?;
    while let Some(effect) = output.effect {
        let result = executor.block_on(identity_transport::receive(
            runtime.clone(),
            effect.binding,
            effect.id.clone(),
        ))?;
        output = invoke(Action::Reply {
            id: effect.id,
            value: result,
        })?;
    }
    if output.view.state != "awaiting_confirmation" {
        return Err("pairing did not return an account".into());
    }
    let account = output.view.account.ok_or("account")?;
    invoke(Action::Confirm { account })?;
    invoke(Action::Grants {
        grants: Grants {
            readable: true,
            relay_auth: vec![relay.clone()],
            ..Default::default()
        },
    })?;
    for method in ["nip04_decrypt", "nip44_decrypt"] {
        let effect = invoke(Action::Probe {
            method: method.into(),
        })?
        .effect
        .ok_or("effect")?;
        let result = executor.block_on(identity_transport::receive(
            runtime.clone(),
            effect.binding,
            effect.id.clone(),
        ))?;
        let output = invoke(Action::Reply {
            id: effect.id,
            value: result,
        })?;
        if output.view.state != "connected" {
            return Err("synthetic decryption failed".into());
        }
    }
    let (mut auth_socket, challenge) = executor.block_on(async {
        let (mut socket, _) = tokio_tungstenite::connect_async(&relay).await?;
        let message = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
            .await?
            .ok_or("no challenge")??;
        let value: serde_json::Value = serde_json::from_str(message.to_text()?)?;
        if value[0] != "AUTH" {
            return Err("no AUTH challenge".into());
        }
        let challenge = value[1].as_str().ok_or("invalid challenge")?.to_owned();
        Ok::<_, Box<dyn std::error::Error>>((socket, challenge))
    })?;
    let effect = runtime.identity_auth(&relay, &challenge, "local-fixture-connection")?;
    let result = executor.block_on(identity_transport::receive(
        runtime.clone(),
        effect.binding,
        effect.id.clone(),
    ))?;
    invoke(Action::Reply {
        id: effect.id,
        value: result,
    })?;
    let auth = runtime
        .identity_take_auth()?
        .ok_or("no verified auth")?
        .auth_message();
    if auth[0] != "AUTH" {
        return Err("invalid auth transport".into());
    }
    executor.block_on(async {
        auth_socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                auth.to_string().into(),
            ))
            .await?;
        let response = tokio::time::timeout(std::time::Duration::from_secs(5), auth_socket.next())
            .await?
            .ok_or("no auth acknowledgement")??;
        let value: serde_json::Value = serde_json::from_str(response.to_text()?)?;
        if value[0] != "OK" || value[1] != auth[1]["id"] || value[2] != true {
            return Err("authentication rejected".into());
        }
        auth_socket.close(None).await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    invoke(Action::Disconnect)?;
    println!("PASS native NIP46 bunker pairing, account confirmation, NIP04/NIP44 decryption, exact kind22242 signature, local disconnect");
    Ok(())
}
