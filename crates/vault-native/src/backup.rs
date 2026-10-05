//! Linux backup orchestration (milestone 05).
use crate::relay::{self, connect, read_subscription, send, Socket};
use crate::vault::{identity_now, BackupLease, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use vault_core::{
    backup::{
        restore_eligible_m05, BackupSnapshot, Binding, MergeInput, Recovery as RestoreReport,
    },
    collection::{
        self, profile_filters, relay_hints_from_metadata, replaceable_winner,
        validate_capture_relay, validate_lookup_grant, validate_lookup_relays, CollectionOutcome,
        CollectionProfile, DiscoverySuggestions, InitialCollectionJob, InitialJobState,
    },
    vault::{Error, Result},
};

static RUNNING: AtomicBool = AtomicBool::new(false);

struct PendingRelay {
    relay: String,
    connection: String,
    _challenge: String,
    socket: Socket,
    sub_id: String,
    profile: CollectionProfile,
    account: String,
    consent_revision: u32,
    vault_token: String,
    vault_generation: u32,
}

static PENDING: Mutex<Option<PendingRelay>> = Mutex::new(None);

fn clear_pending_if_stale(lease: &BackupLease) {
    let mut guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().is_some_and(|p| {
        p.consent_revision != lease.consent_revision
            || p.vault_token != lease.identity.token
            || p.vault_generation != lease.identity.generation
    }) {
        *guard = None;
    }
}

fn drop_pending() {
    if let Ok(mut guard) = PENDING.lock() {
        *guard = None;
    }
}

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
    Read {
        offset: usize,
    },
    Discover {
        #[serde(rename = "lookupRelays")]
        lookup_relays: Vec<String>,
    },
    Capture {
        relay: String,
        #[serde(default = "default_profile")]
        profile: CollectionProfile,
    },
    ResumeCollection,
    CancelCollection,
    StartInitialJob {
        relay: String,
        #[serde(default = "default_profile")]
        profile: CollectionProfile,
    },
    CheckSource,
    Restore {
        relay: String,
        approved: bool,
    },
}

fn default_profile() -> CollectionProfile {
    CollectionProfile::PublicHistory
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePreview {
    pub id: String,
    pub content: String,
    pub created_at: u64,
    pub kind: u16,
}

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
    pub rejected: usize,
    pub notes: Vec<NotePreview>,
    pub outcome: &'static str,
    pub restore: Option<RestoreReport>,
    #[serde(default)]
    pub suggestions: DiscoverySuggestions,
    pub job: Option<InitialCollectionJob>,
}

fn previews(data: &BackupSnapshot, offset: usize) -> Vec<NotePreview> {
    data.events
        .values()
        .skip(offset)
        .take(16)
        .map(|e| NotePreview {
            id: e.id.to_hex(),
            content: if e.kind.as_u16() == 1 {
                e.content.clone()
            } else {
                String::new()
            },
            created_at: e.created_at.as_secs(),
            kind: e.kind.as_u16(),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn output(
    request: &Request,
    data: Option<&BackupSnapshot>,
    offset: usize,
    outcome: CollectionOutcome,
    restore: Option<RestoreReport>,
    suggestions: DiscoverySuggestions,
    job: Option<InitialCollectionJob>,
    rejected: usize,
) -> Output {
    Output {
        request_id: request.request_id.clone(),
        binding: request.binding.clone(),
        account: data.map(|d| d.account.clone()),
        source: data.map(|d| d.source.clone()).unwrap_or_default(),
        captured_at: data.map(|d| d.captured_at).unwrap_or_default(),
        count: data.map(|d| d.events.len()).unwrap_or_default(),
        suppressed: data.map(|d| d.suppressed.len()).unwrap_or_default(),
        excluded: data.map(|d| d.excluded).unwrap_or_default(),
        rejected: data
            .map(|d| d.rejected)
            .unwrap_or(0)
            .saturating_add(rejected),
        notes: data.map(|d| previews(d, offset)).unwrap_or_default(),
        outcome: outcome.as_str(),
        restore: restore.or_else(|| data.and_then(|d| d.recovery.clone())),
        suggestions,
        job,
    }
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
    match tokio::time::timeout(Duration::from_secs(90), execute_inner(&runtime, &request)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Storage),
    }
}

async fn execute_inner(runtime: &Runtime, request: &Request) -> Result<Output> {
    let mut lease = runtime.backup_context(&request.binding)?;
    clear_pending_if_stale(&lease);
    if let Some(mut data) = lease.data.clone() {
        if data.prune_public(identity_now()?) {
            runtime.backup_commit(&request.binding, &lease, &data)?;
            lease = runtime.backup_context(&request.binding)?;
        }
    }
    match &request.action {
        Action::Read { offset } => {
            if *offset > collection::MAX_SUGGESTIONS_PER_PURPOSE * 32 {
                return Err(Error::Limit);
            }
            Ok(output(
                request,
                lease.data.as_ref(),
                *offset,
                CollectionOutcome::OfflineReady,
                None,
                DiscoverySuggestions::default(),
                lease.job.clone(),
                0,
            ))
        }
        Action::CancelCollection => {
            drop_pending();
            Ok(output(
                request,
                lease.data.as_ref(),
                0,
                CollectionOutcome::OfflineReady,
                None,
                DiscoverySuggestions::default(),
                lease.job.clone(),
                0,
            ))
        }
        Action::Discover { lookup_relays } => {
            validate_lookup_relays(lookup_relays)?;
            for relay in lookup_relays {
                validate_lookup_grant(relay, &lease.grants.lookup)?;
            }
            let account = lease.account.as_ref().ok_or(Error::Authentication)?;
            let suggestions =
                discover_metadata(runtime, &request.binding, &lease, lookup_relays, account)
                    .await?;
            Ok(output(
                request,
                lease.data.as_ref(),
                0,
                CollectionOutcome::DiscoveryReady,
                None,
                suggestions,
                lease.job.clone(),
                0,
            ))
        }
        Action::StartInitialJob { relay, profile } => {
            validate_capture_relay(relay, &lease.grants.capture)?;
            let account = lease.account.as_ref().ok_or(Error::Authentication)?;
            let _job = ensure_job(runtime, &request.binding, &lease, account, *profile, relay)?;
            lease = runtime.backup_context(&request.binding)?;
            run_capture(runtime, request, &mut lease, relay, *profile, true).await
        }
        Action::Capture { relay, profile } => {
            validate_capture_relay(relay, &lease.grants.capture)?;
            run_capture(runtime, request, &mut lease, relay, *profile, false).await
        }
        Action::ResumeCollection => resume_pending(runtime, request, &mut lease).await,
        Action::CheckSource => {
            let data = lease.data.as_ref().ok_or(Error::Malformed)?;
            let outcome = match connect(&data.source).await {
                Ok(_) => CollectionOutcome::SourceReachable,
                Err(_) => CollectionOutcome::SourceUnavailable,
            };
            Ok(output(
                request,
                Some(data),
                0,
                outcome,
                None,
                DiscoverySuggestions::default(),
                lease.job.clone(),
                0,
            ))
        }
        Action::Restore { relay, approved } => {
            if !approved {
                return Err(Error::Authentication);
            }
            restore_preview(runtime, request, &lease, relay).await
        }
    }
}

async fn discover_metadata(
    runtime: &Runtime,
    binding: &Binding,
    lease: &BackupLease,
    lookup_relays: &[String],
    account: &str,
) -> Result<DiscoverySuggestions> {
    let mut candidates: BTreeMap<u16, Vec<vault_core::backup::Event>> = BTreeMap::new();
    for lookup in lookup_relays {
        runtime.backup_check(binding, lease)?;
        let mut socket = connect(lookup).await?;
        for kind in [10002_u16, 10050_u16] {
            let sub_id = hex::encode(vault_core::vault::random::<8>(
                &mut crate::vault::OsEntropy,
            )?);
            send(
                &mut socket,
                json!(["REQ", sub_id, {"authors":[account],"kinds":[kind],"limit":5}]),
            )
            .await?;
            let read = read_subscription(
                &mut socket,
                &sub_id,
                |event| {
                    if event.pubkey.to_hex() == account
                        && (event.kind.as_u16() == 10002 || event.kind.as_u16() == 10050)
                    {
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                },
                8,
                128 * 1024,
            )
            .await?;
            candidates.entry(kind).or_default().extend(read.events);
        }
        let _ = send(&mut socket, json!(["CLOSE", "done"])).await;
    }
    let mut history = BTreeSet::new();
    let mut inbox = BTreeSet::new();
    for kind in [10002_u16, 10050_u16] {
        if let Some(events) = candidates.get(&kind) {
            if let Some(winner) = replaceable_winner(events.iter()) {
                let hints = relay_hints_from_metadata(winner, account)?;
                collection::merge_hint_lists(&mut history, &mut inbox, &hints);
            }
        }
    }
    Ok(collection::suggestions_from_maps(&history, &inbox))
}

fn ensure_job(
    runtime: &Runtime,
    binding: &Binding,
    lease: &BackupLease,
    account: &str,
    profile: CollectionProfile,
    relay: &str,
) -> Result<InitialCollectionJob> {
    let now = identity_now()?;
    if let Some(job) = &lease.job {
        if job.consent_revision == lease.consent_revision && job.relay == relay {
            return Ok(job.clone());
        }
    }
    let job = InitialCollectionJob::new(profile, relay.to_string(), lease.consent_revision, now);
    runtime.backup_commit_job(binding, lease, &job)?;
    let _ = account;
    Ok(job)
}

async fn run_capture(
    runtime: &Runtime,
    request: &Request,
    lease: &mut BackupLease,
    relay: &str,
    profile: CollectionProfile,
    from_job: bool,
) -> Result<Output> {
    let account = lease.account.as_ref().ok_or(Error::Authentication)?.clone();
    let current = lease
        .data
        .clone()
        .unwrap_or_else(|| BackupSnapshot::empty(account.clone()));
    if current.account != account {
        return Err(Error::Conflict);
    }
    let read = collect_from_relay(runtime, request, lease, relay, profile, &account, None).await?;
    if read.auth_challenge.is_some() {
        if !lease.grants.relay_auth.iter().any(|u| u == relay) {
            drop_pending();
            return Ok(output(
                request,
                lease.data.as_ref(),
                0,
                CollectionOutcome::NeedsAuthorization,
                None,
                DiscoverySuggestions::default(),
                lease.job.clone(),
                read.rejected,
            ));
        }
        if !lease.signer_connected {
            drop_pending();
            return Ok(output(
                request,
                lease.data.as_ref(),
                0,
                CollectionOutcome::NeedsAuthorization,
                None,
                DiscoverySuggestions::default(),
                lease.job.clone(),
                read.rejected,
            ));
        }
        let challenge = read.auth_challenge.clone().unwrap();
        let connection = PENDING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|p| p.connection.clone())
            .unwrap_or_else(|| {
                hex::encode(vault_core::vault::random::<8>(&mut crate::vault::OsEntropy).unwrap())
            });
        runtime.identity_auth(relay, &challenge, &connection)?;
        return Ok(output(
            request,
            lease.data.as_ref(),
            0,
            CollectionOutcome::WaitingForApproval,
            None,
            DiscoverySuggestions::default(),
            lease.job.clone(),
            read.rejected,
        ));
    }
    if read.denied {
        return Ok(output(
            request,
            lease.data.as_ref(),
            0,
            CollectionOutcome::Denied,
            None,
            DiscoverySuggestions::default(),
            lease.job.clone(),
            read.rejected,
        ));
    }
    commit_capture(runtime, request, lease, relay, profile, read, from_job).await
}

async fn resume_pending(
    runtime: &Runtime,
    request: &Request,
    lease: &mut BackupLease,
) -> Result<Output> {
    let Some(mut pending) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return Err(Error::Malformed);
    };
    if pending.consent_revision != lease.consent_revision {
        return Err(Error::Cancelled);
    }
    let auth = runtime.identity_take_auth()?.ok_or(Error::Authentication)?;
    relay::send_auth(&mut pending.socket, &auth).await?;
    let filters = profile_filters(pending.profile, &pending.account, identity_now()?)?;
    send(&mut pending.socket, json!(["REQ", pending.sub_id, filters])).await?;
    let account = pending.account.clone();
    let prof = pending.profile;
    let read = read_subscription(
        &mut pending.socket,
        &pending.sub_id,
        |event| Ok(collection::matches_profile(&event, &account, prof)),
        vault_core::backup::MAX_EVENTS,
        vault_core::backup::MAX_RAW_BYTES,
    )
    .await?;
    let relay = pending.relay.clone();
    let profile = pending.profile;
    drop(pending);
    if read.auth_challenge.is_some() {
        return Ok(output(
            request,
            lease.data.as_ref(),
            0,
            CollectionOutcome::NeedsAuthorization,
            None,
            DiscoverySuggestions::default(),
            lease.job.clone(),
            read.rejected,
        ));
    }
    commit_capture(runtime, request, lease, &relay, profile, read, false).await
}

async fn collect_from_relay(
    runtime: &Runtime,
    request: &Request,
    lease: &BackupLease,
    relay: &str,
    profile: CollectionProfile,
    account: &str,
    mut existing_socket: Option<Socket>,
) -> Result<relay::RelayRead> {
    runtime.backup_check(&request.binding, lease)?;
    let mut socket = if let Some(s) = existing_socket.take() {
        s
    } else {
        connect(relay).await?
    };
    let sub_id = hex::encode(vault_core::vault::random::<8>(
        &mut crate::vault::OsEntropy,
    )?);
    let filters = profile_filters(profile, account, identity_now()?)?;
    send(&mut socket, json!(["REQ", sub_id, filters])).await?;
    let read = read_subscription(
        &mut socket,
        &sub_id,
        |event| Ok(collection::matches_profile(&event, account, profile)),
        vault_core::backup::MAX_EVENTS,
        vault_core::backup::MAX_RAW_BYTES,
    )
    .await?;
    if read.auth_challenge.is_some() {
        let challenge = read.auth_challenge.clone().unwrap();
        let connection = hex::encode(vault_core::vault::random::<8>(
            &mut crate::vault::OsEntropy,
        )?);
        {
            let mut guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(PendingRelay {
                relay: relay.to_string(),
                connection,
                _challenge: challenge.clone(),
                socket,
                sub_id,
                profile,
                account: account.to_string(),
                consent_revision: lease.consent_revision,
                vault_token: lease.identity.token.clone(),
                vault_generation: lease.identity.generation,
            });
        }
    }
    Ok(read)
}

async fn commit_capture(
    runtime: &Runtime,
    request: &Request,
    lease: &mut BackupLease,
    relay: &str,
    profile: CollectionProfile,
    read: relay::RelayRead,
    from_job: bool,
) -> Result<Output> {
    let account = lease.account.as_ref().ok_or(Error::Authentication)?;
    let current = lease
        .data
        .clone()
        .unwrap_or_else(|| BackupSnapshot::empty(account.clone()));
    let incomplete =
        read.saturated || read.byte_saturated || read.eose_more || read.disconnected || !read.eose;
    let merged = current.merge_incoming(MergeInput {
        source: relay,
        profile,
        events: read.events,
        rejected: read.rejected,
        now: identity_now()?,
        incomplete,
    })?;
    runtime.backup_commit(&request.binding, lease, &merged.snapshot)?;
    *lease = runtime.backup_context(&request.binding)?;
    let mut job = lease.job.clone();
    if from_job {
        if let Some(ref mut j) = job {
            j.state = if merged.stopped || incomplete {
                InitialJobState::Incomplete
            } else {
                InitialJobState::Saved
            };
            j.last_attempt = identity_now()?;
            runtime.backup_commit_job(&request.binding, lease, j)?;
            *lease = runtime.backup_context(&request.binding)?;
        }
    }
    let outcome = if merged.snapshot.events.is_empty() && !incomplete {
        CollectionOutcome::Empty
    } else if merged.stopped || incomplete {
        CollectionOutcome::Incomplete
    } else {
        CollectionOutcome::BackupSaved
    };
    Ok(output(
        request,
        lease.data.as_ref(),
        0,
        outcome,
        None,
        DiscoverySuggestions::default(),
        job,
        0,
    ))
}

async fn restore_preview(
    runtime: &Runtime,
    request: &Request,
    lease: &BackupLease,
    destination: &str,
) -> Result<Output> {
    let data = lease.data.as_ref().ok_or(Error::Malformed)?;
    if destination == data.source || data.events.is_empty() {
        return Err(Error::Malformed);
    }
    vault_core::identity::relay(destination)?;
    let now = identity_now()?;
    let mut socket = connect(destination).await?;
    let mut report = RestoreReport {
        destination: destination.to_string(),
        ..Default::default()
    };
    let mut ids = BTreeSet::new();
    for event in data.events.values() {
        runtime.backup_check(&request.binding, lease)?;
        if !restore_eligible_m05(event, &data.suppressed, now) {
            continue;
        }
        report.attempted += 1;
        ids.insert(event.id.to_hex());
        send(&mut socket, json!(["EVENT", event])).await?;
        for _ in 0..32 {
            let value = match relay::receive(&mut socket).await {
                Ok(v) => v,
                Err(_) => break,
            };
            if value.get(0).and_then(|v| v.as_str()) == Some("AUTH") {
                return Ok(output(
                    request,
                    Some(data),
                    0,
                    CollectionOutcome::NeedsAuthorization,
                    None,
                    DiscoverySuggestions::default(),
                    lease.job.clone(),
                    0,
                ));
            }
            if value.get(0).and_then(|v| v.as_str()) == Some("OK")
                && value.get(1).and_then(|v| v.as_str()) == Some(&event.id.to_hex())
            {
                if value.get(2) == Some(&json!(true)) {
                    report.acknowledged += 1;
                } else if value.get(2) == Some(&json!(false)) {
                    report.rejected += 1;
                }
                break;
            }
        }
    }
    drop(socket);
    if !ids.is_empty() {
        if let Ok(read) = collect_from_relay(
            runtime,
            request,
            lease,
            destination,
            CollectionProfile::PublicHistory,
            &data.account,
            None,
        )
        .await
        {
            let returned: BTreeSet<_> = read.events.iter().map(|e| e.id.to_hex()).collect();
            report.verified = ids.intersection(&returned).count();
            report.verification_complete = read.eose;
        }
    }
    report.checked_at = now;
    let state = if report.attempted > 0 && report.verified == report.attempted {
        CollectionOutcome::RestoreVerified
    } else {
        CollectionOutcome::RestorePartial
    };
    let mut saved = data.clone();
    saved.recovery = Some(report.clone());
    runtime.backup_commit(&request.binding, lease, &saved)?;
    Ok(output(
        request,
        Some(&saved),
        0,
        state,
        Some(report),
        DiscoverySuggestions::default(),
        lease.job.clone(),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::backup::BackupSnapshot;
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
    fn encrypted_restart_preserves_snapshot_without_signer() {
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
        let snap_binding = binding(&status);
        let lease = runtime.backup_context(&snap_binding).unwrap();
        let note: vault_core::backup::Event =
            serde_json::from_str(include_str!("../../../tests/fixtures/public-note.json")).unwrap();
        let merged = BackupSnapshot::empty(note.pubkey.to_hex())
            .merge_incoming(MergeInput {
                source: "ws://127.0.0.1:1",
                profile: CollectionProfile::PublicHistory,
                events: vec![note],
                rejected: 0,
                now: 200,
                incomplete: false,
            })
            .unwrap()
            .snapshot;
        runtime
            .backup_commit(&snap_binding, &lease, &merged)
            .unwrap();
        drop(runtime);
        let runtime = Runtime::new(dir.clone()).unwrap();
        let locked = runtime.execute(&request(None, Operation::Status)).unwrap();
        let status = runtime
            .execute(&request(
                Some(&locked),
                Operation::Unlock {
                    password: "disposable backup password".into(),
                },
            ))
            .unwrap();
        let snap_binding = binding(&status);
        let lease = runtime.backup_context(&snap_binding).unwrap();
        assert_eq!(lease.data.unwrap().events.len(), 1);
        assert!(lease.account.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
