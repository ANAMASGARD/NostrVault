//! Linux MVP transport and encrypted snapshot orchestration. Only public notes.
use crate::vault::{identity_now, BackupLease, Runtime};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};
use vault_core::{
    backup::{Backup, Binding, Event, LIMIT},
    vault::{Error, Result},
};
static RUNNING: AtomicBool = AtomicBool::new(false);
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub binding: Binding,
    pub action: Action,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Read { offset: usize },
    Capture { relay: String, approved: bool },
    CheckSource,
    Restore { relay: String, approved: bool },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub content: String,
    pub created_at: u64,
}
pub use vault_core::backup::Recovery as RestoreReport;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub request_id: String,
    pub binding: Binding,
    pub account: Option<String>,
    pub source: String,
    pub captured_at: u64,
    pub count: usize,
    pub suppressed: usize,
    pub excluded: usize,
    pub notes: Vec<Note>,
    pub outcome: &'static str,
    pub restore: Option<RestoreReport>,
}
fn output(
    request: &Request,
    data: Option<&Backup>,
    offset: usize,
    outcome: &'static str,
    restore: Option<RestoreReport>,
) -> Output {
    Output {
        request_id: request.request_id.clone(),
        binding: request.binding.clone(),
        account: data.map(|d| d.account.clone()),
        source: data.map(|d| d.source.clone()).unwrap_or_default(),
        captured_at: data.map(|d| d.captured_at).unwrap_or_default(),
        count: data.map(|d| d.notes.len()).unwrap_or_default(),
        suppressed: data.map(|d| d.suppressed.len()).unwrap_or_default(),
        excluded: data.map(|d| d.excluded).unwrap_or_default(),
        notes: data
            .map(|d| {
                d.notes
                    .values()
                    .skip(offset)
                    .take(16)
                    .map(|e| Note {
                        id: e.id.to_hex(),
                        content: e.content.clone(),
                        created_at: e.created_at.as_secs(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        outcome,
        restore: restore.or_else(|| data.and_then(|d| d.recovery.clone())),
    }
}
async fn connect(relay: &str) -> Result<Socket> {
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
async fn send(socket: &mut Socket, value: Value) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(3),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| Error::Storage)?
    .map_err(|_| Error::Storage)
}
async fn receive(socket: &mut Socket) -> Result<Value> {
    let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .map_err(|_| Error::Storage)?
        .ok_or(Error::Storage)?
        .map_err(|_| Error::Storage)?;
    match message {
        Message::Text(text) => serde_json::from_str(&text).map_err(|_| Error::Malformed),
        Message::Ping(_) | Message::Pong(_) => Ok(Value::Null),
        _ => Err(Error::Storage),
    }
}
async fn query(
    runtime: &Runtime,
    binding: &Binding,
    lease: &BackupLease,
    relay: &str,
    account: &str,
    ids: Option<&BTreeSet<String>>,
) -> Result<Vec<Event>> {
    let mut socket = connect(relay).await?;
    runtime.backup_check(binding, lease)?;
    let id = hex::encode(vault_core::vault::random::<16>(
        &mut crate::vault::OsEntropy,
    )?);
    let mut filter =
        json!({"authors":[account],"kinds":[1,5],"limit":LIMIT,"until":identity_now()?});
    if let Some(ids) = ids {
        filter = json!({"ids":ids,"authors":[account],"kinds":[1],"limit":LIMIT});
    }
    send(&mut socket, json!(["REQ", id, filter])).await?;
    let mut events = Vec::new();
    for _ in 0..1024 {
        runtime.backup_check(binding, lease)?;
        let value = receive(&mut socket).await?;
        match value[0].as_str() {
            Some("EOSE") if value[1] == id => {
                let _ = send(&mut socket, json!(["CLOSE", id])).await;
                return Ok(events);
            }
            Some("EVENT") if value[1] == id => {
                if value.as_array().map(Vec::len) != Some(3) {
                    return Err(Error::Malformed);
                }
                let event: Event =
                    serde_json::from_value(value[2].clone()).map_err(|_| Error::Malformed)?;
                vault_core::backup::validate(&event, account)?;
                if event.created_at.as_secs() > identity_now()?
                    || ids.is_some_and(|ids| !ids.contains(&event.id.to_hex()))
                {
                    return Err(Error::Authentication);
                }
                events.push(event);
                if events.len() >= LIMIT {
                    return Err(Error::Limit);
                }
            }
            Some("AUTH") | Some("CLOSED") => return Err(Error::Unsupported),
            _ => {}
        }
    }
    Err(Error::Limit)
}
pub async fn execute(runtime: Arc<Runtime>, request: Request) -> Result<Output> {
    if !cfg!(target_os = "linux") {
        return Err(Error::Unsupported);
    }
    if request.request_id.is_empty() || request.request_id.len() > 64 {
        return Err(Error::Malformed);
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(Error::Busy);
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    tokio::time::timeout(Duration::from_secs(45), execute_inner(&runtime, &request))
        .await
        .map_err(|_| Error::Storage)?
}
async fn execute_inner(runtime: &Runtime, request: &Request) -> Result<Output> {
    let mut lease = runtime.backup_context(&request.binding)?;
    if let Some(mut data) = lease.data.clone() {
        if data.prune(identity_now()?) {
            runtime.backup_commit(&request.binding, &lease, &data)?;
            lease = runtime.backup_context(&request.binding)?;
        }
    }
    match &request.action {
        Action::Read { offset } => {
            if *offset > LIMIT {
                return Err(Error::Limit);
            }
            Ok(output(
                request,
                lease.data.as_ref(),
                *offset,
                "offline_ready",
                None,
            ))
        }
        Action::CheckSource => {
            let data = lease.data.as_ref().ok_or(Error::Malformed)?;
            let state = match connect(&data.source).await {
                Ok(_) => "source_reachable",
                Err(_) => "source_unavailable",
            };
            runtime.backup_check(&request.binding, &lease)?;
            Ok(output(request, Some(data), 0, state, None))
        }
        Action::Capture { relay, approved } => {
            if !approved {
                return Err(Error::Authentication);
            }
            let account = lease.account.as_ref().ok_or(Error::Authentication)?;
            let current = lease
                .data
                .clone()
                .unwrap_or_else(|| Backup::empty(account.clone()));
            if current.account != *account {
                return Err(Error::Conflict);
            }
            let events = query(runtime, &request.binding, &lease, relay, account, None).await?;
            let data = current.merge(relay, events, identity_now()?)?;
            runtime.backup_commit(&request.binding, &lease, &data)?;
            let committed = runtime.backup_context(&request.binding)?;
            Ok(output(
                request,
                committed.data.as_ref(),
                0,
                "backup_saved",
                None,
            ))
        }
        Action::Restore { relay, approved } => {
            if !approved {
                return Err(Error::Authentication);
            }
            let data = lease.data.as_ref().ok_or(Error::Malformed)?;
            if relay == &data.source || data.notes.is_empty() {
                return Err(Error::Malformed);
            }
            // Learn any destination deletion evidence before forwarding. This is a
            // fresh request, never a cache read. Source-side unseen deletions remain a gap.
            let observed = query(
                runtime,
                &request.binding,
                &lease,
                relay,
                &data.account,
                None,
            )
            .await?;
            let deletions = observed
                .into_iter()
                .filter(|e| e.kind.as_u16() == 5)
                .collect();
            let mut merged = data.merge(&data.source, deletions, identity_now()?)?;
            merged.captured_at = data.captured_at;
            let data = merged;
            runtime.backup_commit(&request.binding, &lease, &data)?;
            lease = runtime.backup_context(&request.binding)?;
            let data = lease.data.as_ref().ok_or(Error::Storage)?;
            let mut socket = connect(relay).await?;
            let mut report = RestoreReport {
                destination: relay.clone(),
                ..Default::default()
            };
            let mut ids = BTreeSet::new();
            for event in data.notes.values() {
                runtime.backup_check(&request.binding, &lease)?;
                if !vault_core::backup::eligible(event, identity_now()?) {
                    continue;
                }
                report.attempted += 1;
                ids.insert(event.id.to_hex());
                send(&mut socket, json!(["EVENT", event])).await?;
                for _ in 0..32 {
                    let value = match receive(&mut socket).await {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    runtime.backup_check(&request.binding, &lease)?;
                    if value[0] == "AUTH" {
                        return Err(Error::Unsupported);
                    }
                    if value[0] == "OK" && value[1] == event.id.to_hex() {
                        if value[2] == true {
                            report.acknowledged += 1;
                        } else if value[2] == false {
                            report.rejected += 1;
                        }
                        break;
                    }
                }
            }
            drop(socket);
            // A NEW socket/request verifies bodies, IDs and signatures independently.
            if !ids.is_empty() {
                if let Ok(events) = query(
                    runtime,
                    &request.binding,
                    &lease,
                    relay,
                    &data.account,
                    Some(&ids),
                )
                .await
                {
                    let returned: BTreeSet<_> = events.iter().map(|e| e.id.to_hex()).collect();
                    report.verified = ids.intersection(&returned).count();
                    report.verification_complete = true;
                }
            }
            runtime.backup_check(&request.binding, &lease)?;
            let state = if report.attempted > 0 && report.verified == report.attempted {
                "restore_verified"
            } else {
                "restore_partial"
            };
            report.checked_at = identity_now()?;
            let mut saved = data.clone();
            saved.recovery = Some(report.clone());
            runtime.backup_commit(&request.binding, &lease, &saved)?;
            Ok(output(request, Some(&saved), 0, state, Some(report)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::vault::{Operation, Request as VaultRequest, Status};
    fn request(status: Option<&Status>, operation: Operation) -> VaultRequest {
        VaultRequest {
            version: 1,
            request_id: "public-backup-test".into(),
            token: status.map(|s| s.token.clone()).unwrap_or_default(),
            generation: status.map(|s| s.generation).unwrap_or_default(),
            vault_id: status.and_then(|s| s.vault_id.clone()),
            operation,
        }
    }
    fn binding(status: &Status) -> Binding {
        Binding {
            vault_id: status.vault_id.clone().unwrap(),
            token: status.token.clone(),
            generation: status.generation,
        }
    }
    #[test]
    fn encrypted_restart_revision_and_lock_preserve_committed_notes() {
        let dir = std::env::temp_dir().join(format!("nv-backup-fixture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let runtime = Runtime::new(dir.clone()).unwrap();
        let initial = runtime.execute(&request(None, Operation::Status)).unwrap();
        let status = runtime
            .execute(&request(
                Some(&initial),
                Operation::Create {
                    password: "disposable backup password".into(),
                    confirmation: "disposable backup password".into(),
                },
            ))
            .unwrap();
        let binding = binding(&status);
        let lease = runtime.backup_context(&binding).unwrap();
        let note: Event =
            serde_json::from_str(include_str!("../../../tests/fixtures/public-note.json")).unwrap();
        let data = Backup::empty(note.pubkey.to_hex())
            .merge("ws://127.0.0.1:1", vec![note], 200)
            .unwrap();
        runtime.backup_commit(&binding, &lease, &data).unwrap();
        assert_eq!(
            runtime.backup_commit(&binding, &lease, &data).err(),
            Some(Error::Conflict)
        );
        let lease = runtime.backup_context(&binding).unwrap();
        runtime.invalidate();
        assert_eq!(
            runtime.backup_commit(&binding, &lease, &data).err(),
            Some(Error::Cancelled)
        );
        drop(runtime);
        let bytes = std::fs::read(dir.join("vault.sqlite")).unwrap();
        assert!(!bytes
            .windows(b"NV_PUBLIC_UNIT_FIXTURE".len())
            .any(|w| w == b"NV_PUBLIC_UNIT_FIXTURE"));
        let runtime = Runtime::new(dir.clone()).unwrap();
        let locked = runtime.execute(&request(None, Operation::Status)).unwrap();
        assert!(runtime.backup_context(&binding).is_err());
        let status = runtime
            .execute(&request(
                Some(&locked),
                Operation::Unlock {
                    password: "disposable backup password".into(),
                },
            ))
            .unwrap();
        let lease = runtime
            .backup_context(&super::tests::binding(&status))
            .unwrap();
        assert_eq!(lease.data.unwrap().notes.len(), 1);
        assert!(
            lease.account.is_none(),
            "Offline reads do not require signer connection"
        );
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn acknowledged_but_not_returned_is_never_verified() {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        executor.block_on(async {
            let dir =
                std::env::temp_dir().join(format!("nv-backup-readback-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let runtime = Arc::new(Runtime::new(dir.clone()).unwrap());
            let initial = runtime.execute(&request(None, Operation::Status)).unwrap();
            let status = runtime
                .execute(&request(
                    Some(&initial),
                    Operation::Create {
                        password: "readback fixture password".into(),
                        confirmation: "readback fixture password".into(),
                    },
                ))
                .unwrap();
            let binding = binding(&status);
            let note: Event =
                serde_json::from_str(include_str!("../../../tests/fixtures/public-note.json"))
                    .unwrap();
            let data = Backup::empty(note.pubkey.to_hex())
                .merge("ws://127.0.0.1:1", vec![note], 200)
                .unwrap();
            let lease = runtime.backup_context(&binding).unwrap();
            runtime.backup_commit(&binding, &lease, &data).unwrap();
            for completed in [true, false] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let relay = format!("ws://{}", listener.local_addr().unwrap());
                let server = tokio::spawn(async move {
                    for connection in 0..3 {
                        let (stream, _) = listener.accept().await.unwrap();
                        let mut socket =
                            tokio_tungstenite::accept_async(MaybeTlsStream::Plain(stream))
                                .await
                                .unwrap();
                        while let Some(Ok(Message::Text(text))) = socket.next().await {
                            let v: Value = serde_json::from_str(&text).unwrap();
                            if v[0] == "REQ" {
                                if connection == 2 && !completed {
                                    break;
                                }
                                send(&mut socket, json!(["EOSE", v[1]])).await.unwrap();
                            } else if v[0] == "EVENT" {
                                send(&mut socket, json!(["OK", v[1]["id"], true, ""]))
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                });
                let result = execute(
                    runtime.clone(),
                    Request {
                        request_id: "readback-negative".into(),
                        binding: binding.clone(),
                        action: Action::Restore {
                            relay,
                            approved: true,
                        },
                    },
                )
                .await
                .unwrap();
                server.await.unwrap();
                let report = result.restore.unwrap();
                assert_eq!(report.acknowledged, 1);
                assert_eq!(report.verified, 0);
                assert_eq!(report.verification_complete, completed);
                assert_eq!(result.outcome, "restore_partial");
            }
            drop(runtime);
            std::fs::remove_dir_all(dir).unwrap();
        });
    }
}
