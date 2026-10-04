# NostrVault — 15-Commit Implementation Plan

## Status, scope, and use

**Approved for incremental implementation, 2026-10-04.** The amendments below record the revised plan approved in the conversation without replacing this template or adding milestones. They supersede conflicting original wording. Implementation evidence lives in `MEMORY/memory.md`; approval is not test evidence.

### Approved revisions and milestone mapping

- **01 — `build: establish multiplatform verification and product contracts`:** Establish pinned toolchains, dependency locks, format/type/lint/unit tooling, CI, actual web/Linux/Android builds and launch tests, a minimal real Rust/WASM probe invoked in a browser Worker with typed UI results and missing/corrupt/unloadable-module failure tests in Chromium and Firefox, and the product/security contracts. All automated gates run before each milestone commit; missing prerequisites block the commit.
- **02 — `feat(core): establish shared engine and platform contracts`:** Prove native/WASM library parity, typed Worker/native contracts, storage conformance, archive crypto vectors, and Android headless JNI. Tooling/platform initialization belongs in 01, not deferred to 02.
- **03–05:** Implement resumable first-run flow: connect an existing account → protect vault → approve source relays/permissions → review → start. Preserve back-navigation state and handle absent/incompatible signer, wrong account, denial, cancellation, and interrupted setup without requesting an identity secret. Browser, Linux remote pairing, and Android installed-signer flows use platform-appropriate wording. Public-history and existing-archive paths remain accessible without a signer.
- **03, 08:** Default new archives to the vault password, with explicit export-password override. Old archives retain their original password after a vault password change. Explain loss consequences; signer reconnection is not password recovery. Open archives within NostrVault without CLI tools. Standard encrypted archives include consented, already-decoded history.
- **04:** Typed Worker-to-main-page NIP-07 broker; bind responses to request, account, and vault-session generation. Bound concurrency, cancel on lock/logout/account switch, discard stale replies, and pause on denial. Test real signer products/versions and permissions; never promise universal one-time authorization.
- **05:** Disclose and approve lookup relays before discovery (initial suggestions: `wss://relay.damus.io`, `wss://nos.lol`; maximum three). Query only selected-account metadata; distinguish kind 10002 from kind 10050, show at most ten history suggestions, never crawl recursively, and separately approve history sources. Support validated manual entry. Distinguish invalid address, unavailable relay, authentication, denied access, successful bounded empty reads, and incomplete collection. NIP-11 hints do not prove compatibility; no synthetic public writes.
- **05–07:** Start approved capture automatically and idempotently after onboarding; persist grants/job state without duplicate jobs. Until 07 delivers continuous capture, label 05's initial collection honestly. Capture, decode, replication, and archive completion remain separate. A new contact creates a conversation only after authorized validated decoding.
- **06:** First product checkpoint on each platform: fresh install → real signer → approved relay → real captured/decrypted message → restart → readable saved message with network and signer disabled. Include a minimal reader here. Benchmark 1,000-message bulk history with actual signer versions, recording prompts, timing, memory, cancellation/resume, and unattended limitations.
- **07, 12–14:** Show truthful platform/lock states. Strict lock pauses by default; optional native sealed capture cannot update readable conversations. Browser suspension and actual process exit stop continuous execution. Android catch-up defaults to approximately 30 minutes under OS scheduling. Linux changed-vault snapshots default to hourly while unlocked, retaining five app-managed snapshots only after successful replacement.
- **08–09:** Imported readable conversations remain **archive-derived** until independently validated against original encrypted messages through an authorized signer. Archive decryption/integrity does not prove original authorship. Imported verification claims do not upgrade trust. Preserve signer-free offline reading with provenance labels. Test a validly encrypted archive whose transcript disagrees with its original messages; it must remain unverified and fail later cross-validation.
- **09:** Overview, Conversations, Backups, Settings navigation; consistent Working, Waiting for approval, Paused, Needs attention, Ready offline states; protocol details under Advanced. Distinguish local storage from an independent copy. Define typography, spacing, responsive/keyboard behavior, loading/empty/error states, and only wired actions. Conduct uncoached tasks with at least three unfamiliar people; record failures and retest fixes.
- **10, 15:** Private replication/restoration follows the approved recipient-inbox policy and required authentication. Keep sender self-copies and recipient wraps separate; never publish plaintext rumors or use public fan-out. The outage demo explicitly distinguishes signer-free offline archive reading from operations needing signer access or separately authorized routing updates. Test signer-unavailable restoration pauses, denied AUTH, and destinations outside policy. Never silently update kind 10002/10050.
- **01, 04, 09, 12, 15:** Named CSP, native capability, validated IPC, safe external-link and untrusted-rendering checks; clean-install journeys on each platform. Hardware/real-signer/usability gates occur at their relevant milestones and release. A CI definition or mock adapter is not evidence those gates passed.

The original milestone bodies below remain applicable except where these approved revisions override them. Milestones 11–15 retain their original feature scope. No push, public deployment, release publication, or production credentials without separate approval. Local milestone commits are authorized only after required checks pass.

This is a proposed implementation roadmap, not an implementation report or security certification. The repository manifests inspected for this plan still describe the React/TypeScript/Vite + Tauri 2 starter. The attached AGENTS.md describes an initial public-history, desktop-first product; this request explicitly expands that scope to private-chat backup, offline viewing, web, Linux, and Android. Commit 01 must record the expansion rather than silently contradict the instructions.

Use the current checkout as the authority for implementation status. Preserve unrelated changes. These are 15 reviewable milestones suitable for squash commits; they are not 15 trivial edits and are not a guarantee of finishing by a hackathon deadline.

Suggested feature branch: `feat/multiplatform-chat-vault`.

**Product:** A local-first Nostr backup and recovery application that collects supported history from authorized relays, preserves encrypted originals, maintains selected replicas, and opens a protected, searchable conversation archive even when the source relay is offline.

**Primary deliverables:** Web/PWA build, Linux Tauri application, Android Tauri application, one documented portable archive format, one shared policy engine, and reproducible recovery tests.

**Not this release:** A new messenger; WhatsApp/Telegram account import; universal support for every Nostr client; NIP-29 community administration migration; MLS/Marmot key-state recovery; a DHT/search network; a hosted custody service; a Tor/anonymity guarantee; or guaranteed synchronization when no authorized process can run.

The WhatsApp/Telegram comparison describes the organization and usability of backups, not file-format interoperability or access to those services.

## A. What the product actually promises

### A1. Supported content

| Content | Initial behavior | Important boundary |
| --- | --- | --- |
| Public profile, notes, follow/relay metadata | Scoped collection, archive, lifecycle-aware replication and restore | No claim of globally complete history |
| NIP-04 legacy direct messages | Capture incoming/outgoing originals; authorized decryption; archive/view | NIP-04 is deprecated; support for old history, not new messaging |
| NIP-17 / NIP-59 private messages | Capture kind-1059 wraps addressed to the user; unwrap via authorized signer; show verified messages | An author-only scan cannot collect these; missing self-copies are not reconstructible |
| NIP-17 small group conversations | Group by the authenticated sender plus participant set, as defined by NIP-17 | Not the same as NIP-29 relay groups or MLS groups |
| NIP-17 file messages | Preserve encrypted metadata; explicitly permitted attachment retrieval in Commit 11 | A saved URL is not a saved attachment |
| Other kinds or chat schemes | Explicit unsupported state; no automatic public forwarding | Adding support requires protocol-specific authorization and lifecycle rules |

Preserve original signed envelopes. Decrypted NIP-17 message rumors are not independently signed events and must never be published as plaintext for restoration. Validate wrapper/seal signatures, rumor hash, and the match between the seal's signer and rumor's claimed author.

### A2. Platform guarantees

| Platform | While active | Away from the UI | Offline viewer |
| --- | --- | --- | --- |
| Web/PWA | Live subscriptions and incremental capture while execution is available | Resume and catch up after suspension/reopen; no guaranteed closed-browser daemon | Previously materialized messages and saved attachments, after unlocking |
| Linux | Live capture and processing | Optional running background process/tray/autostart; stops on actual exit or shutdown | Yes, for retained local data |
| Android | Live capture and processing while app is active | WorkManager catch-up with OS constraints; minimum periodic interval is 15 minutes, execution can be delayed | Yes, for retained local data |

A service worker does not provide an indefinitely running WebSocket worker. Android WorkManager is not a real-time timer. A foreground service is not an unlimited escape hatch: current Android versions impose limits on data-sync foreground services. Force-stop, revoked permission, unavailable signer, locked storage, or no network must produce truthful paused/blocked/stale states.

A ZIP file already downloaded from a website cannot be silently updated everywhere. Web maintains its local encrypted vault and exports snapshots on request; write-back to a chosen file/folder is an optional, capability-detected permission flow.

### A3. User permission is several permissions

1. Choose an account and explicit source relays. A public key alone does not prove ownership or allow private decryption.
2. Connect a signer: NIP-07 browser extension on web, NIP-46 remote signer on Linux, NIP-55 Android signer on Android. Offer NIP-46 as another route where supported.
3. Request only required methods: public-key retrieval, the selected NIP-04/NIP-44 decrypt capability, and narrowly scoped authentication signing when needed. Relay-list changes need separate approval.
4. Choose whether decoded chats may be retained locally under vault encryption. Ciphertext-only and readable-offline modes must be distinct.
5. Choose attachment retrieval, replica destinations, background behavior, and snapshot retention separately.
6. Pause/revoke must stop queued future actions. It cannot withdraw data already published or erase previously exported copies.

NIP-07 decrypt methods are optional. NIP-55 background Content Resolver access depends on remembered permissions. Do not claim one NostrVault consent switch overrides external signer decisions. Broad remembered decrypt permissions have privacy implications; describe them and provide a strict interactive alternative.

### A4. Data flow

```text
Authorized source relays
  -> historical collection + live subscription
  -> outer event validation and scope checks
  -> durable encrypted raw-event storage / locked capture spool
  -> authorized decryption, if available
  -> inner-message verification and conversation projection
  -> encrypted conversation records and local viewer
  -> durable replication queue for eligible original events
  -> consistent encrypted archive snapshots
```

When a new person messages the user, a valid self-addressed gift wrap enters the same queue. After authorized decryption, participant keys select/create the conversation. Profile names are display labels, not identifiers. If decryption is unavailable, display a count of pending encrypted messages rather than inventing names or claiming the readable backup is current.

## B. Architecture decision

Retain the Rust-owned core proposed in AGENTS.md, but make it independent of Tauri and native-only APIs. Compile the portable core to WebAssembly for the web. Do not implement a second competing backup engine in React.

```text
Shared React UI
  |-- Web: Worker + WASM bridge
  |       -> browser WebSocket / IndexedDB / file adapters
  |
  |-- Linux/Android: typed Tauri bridge
          -> Rust native runtime / SQLite / filesystem adapters
          -> Android Kotlin plugin -> WorkManager and NIP-55
                                   -> JNI -> same headless Rust runtime

Shared portable Rust core:
validation, lifecycle policy, collection planning, job transitions,
message projection, archive contracts, restore planning and evidence
```

Suggested boundaries, not directories to create without need:

```text
src/features/{onboarding,backup,chats,restore,settings}/
src/platform/web/
src/platform/tauri/
src/worker/
crates/vault-core/
crates/vault-native/
crates/vault-wasm/
src-tauri/
plugins/nostrvault-mobile/android/
tests/fixtures/
tests/e2e/
docs/{architecture,security,archive-format,compatibility,demo}.md
```

The shared engine owns policy and state transitions. Native and browser hosts perform I/O and persist transaction results through narrow ports. Native code may use Tokio; browser builds must not import Tokio-only networking, native SQLite, Tauri, keyrings, or filesystem APIs into the portable crate.

Use a maintained Nostr implementation for canonical validation and cryptographic primitives. The Rust `nostr` crate documents WASM support, but that does not prove every desired crate, feature, signer, or transport builds on every target. Prove the selected dependency combination in Commit 02.

## C. Storage, archives, and security decisions

### C1. Database first, ZIP second

Keep a transactional incremental vault as the authoritative store. Never rewrite a full archive for every message. Native storage is SQLite with encrypted sensitive record payloads; browser storage is IndexedDB behind the same behavioral contract.

Logical records:

- Accounts and versioned consent policies.
- Validated raw events and provenance.
- Authenticated conversations and messages.
- Profile snapshots and optional local aliases.
- Relay/filter coverage and collection errors.
- Durable processing, replication, and snapshot jobs.
- Attachment availability, integrity, and storage references.
- Restore plans and timestamped per-destination evidence.

Use opaque or keyed lookup identifiers where practical. Encrypt message bodies, names, profile snapshots, private participant data, attachment metadata/keys, and sensitive configuration. Do not create a plaintext full-text index outside the vault. Record counts, ciphertext sizes, timing, and some operational metadata may remain observable; document this instead of promising zero metadata leakage.

### C2. Key separation and locked capture

Use a random vault data key with authenticated encryption. Wrap it with a password-derived key using an established memory-hard KDF/library, with bounded parameters. Device convenience unlock is optional, using an actual protected native key store. Stronghold or a keyring is not, by itself, encryption of every database row.

Do not derive backup passwords from npubs. Do not store the Nostr identity private key in archives. Store NIP-46 session credentials as secrets and destroy them on disconnect; they are not the user's identity key.

Lock clears decoded state and keys as far as the runtime permits. Do not promise perfect JavaScript-memory erasure. Decrypted UI content is exposed to the running application and a compromised device.

For explicitly enabled locked native capture, use a separate bounded encrypted ingress spool. Prefer recipient-based encryption to a backup-ingress public key; its decrypting key remains inside the unlocked vault. The background worker can append sealed raw-event batches without decrypting chat contents. Minimal required operational configuration needs its own documented OS-protected storage policy. Authentication may still require a remembered signer grant; otherwise pause with `needs_authorization`.

Strict mode disables locked capture. Reopening drains and validates the spool transactionally. Spool failures, disk limits, duplicate batches and interrupted merges must not advance authoritative processing coverage incorrectly. Captured, decoded and snapshot coverage are separate values.

### C3. Portable archive

Recommended default: a ZIP payload encrypted as a whole using an existing standard authenticated streaming envelope, such as age v1, with a compatible library verified on all three targets. Do not implement a new cipher, obsolete ZipCrypto, or a home-grown encrypted chunk protocol. The proposed release filename is `nostrvault-<date>-<snapshot-id>.zip.age`; final dependency/format selection is frozen after Commit 02's interoperability spike.

The structure below is visible only AFTER decryption:

```text
manifest.json
profiles.json
raw/events.jsonl
raw/provenance.jsonl
conversations/<stable-id>/metadata.json
conversations/<stable-id>/messages.jsonl
conversations/<stable-id>/chat.txt
readable/Alice__<key-prefix>/chat.txt
readable/Bob__<key-prefix>/chat.txt
media/<content-hash>.<safe-extension>
reports/coverage.json
reports/restore-eligibility.json
```

Store full participant public keys in encrypted metadata. Names may change, collide, contain path separators, or impersonate another person. Use sanitized names only as display aliases; stable IDs remain canonical. Avoid names in the outer filename. Generate HTML only if it is entirely self-contained and safely escaped; plain text is enough initially.

The manifest includes versions, snapshot ID, covered account/scope/time, source outcomes, raw-event and logical-message counts, attachment results, hashes, exclusions and decoding coverage. It contains no credentials. Hashes detect inconsistent contents; do not claim an unsigned manifest proves independent authorship.

Readable export is optional. A password unlocks already-materialized transcripts, not arbitrary undecrypted Nostr ciphertext. Preserve both original envelopes and cached projections; label transcript provenance and rederive projections when an authorized signer is available.

Export from a consistent database checkpoint. Native snapshots use staged output plus atomic finalization where the filesystem supports it. Android document providers and browser downloads may not support atomic overwrite: create a new completed snapshot, verify it, then apply user-approved retention. Never delete the last known-good backup first.

Auto-snapshot generation runs only when its required keys and execution capability are available. Show `last captured`, `last decoded` and `last archive snapshot` separately. Provide an explicit, warned plain ZIP export for users who deliberately need readable unencrypted files.

## D. The 15 commits

### 01 — `build: establish multiplatform verification and product contracts`

**Build:** Update AGENTS.md and the README to include web/PWA, Linux and Android; add private-chat scope, offline viewer, the background capability matrix, consent boundaries, event-kind support, retention/deletion behavior and the core ownership ADR. Record NIP-29 and MLS as separate unsupported protocols, not all-purpose DMs.

**Deliverables:** A compact architecture ADR, initial threat model, compatibility matrix, real feature checklist and updated memory. Keep facts and proposals separate. Preserve `com.nostrvault.app` unless an explicitly approved identifier change is needed.

**Gate:** Every requested capability maps to an implementation milestone and platform condition. No document promises all history, perpetual storage, invisible background execution, or universal client support. The tests will measure known fixture coverage, not global completeness.

### 02 — `feat(core): establish shared engine and platform contracts`

**Build:** Create the minimal portable/native/WASM boundaries; retain the existing React frontend and npm. Add dependency locks, formatting, type checks, unit-test tooling and CI. Initialize Android early rather than discovering toolchain failures in the final commit. Add typed platform adapters with identical observable contracts.

**Spike:** In Linux native, Android native and a real browser Worker, validate the same signed fixture, perform a protected payload round-trip, write/read an adapter record and exchange a typed result with React. Test the chosen archive-encryption library on WASM before freezing the format. An Android JNI headless entrypoint smoke test belongs here, before background scheduling is promised.

**Gate:** Browser execution contains no Tauri IPC dependency. Android installs and opens. Linux opens. The same canonical validation vectors pass in native and WASM. Commit only reproducible dependencies; do not substitute a mock browser engine for an unsupported dependency without revising the ADR.

### 03 — `feat(vault): add encrypted durable storage and lock lifecycle`

**Build:** Implement SQLite and IndexedDB adapters, schema migrations, transactional record batches, vault creation/unlock/lock, password wrapping and bounded streaming record access. Add per-account job isolation and single-writer coordination. The store should be sufficient for later jobs, not a speculative universal database abstraction.

**Security:** Encrypt sensitive payloads and names before persistence. Add the encrypted ingress-spool format and strict-lock policy. Keep device unlock opt-in. Prevent private values entering logs, error objects, telemetry, caches or plaintext full-text indexes. Use maintained libraries and fail closed when protected native key storage is unavailable.

**Gate:** Restart retains data; a wrong password and tampering fail; interrupted migration preserves the old usable vault; disk/quota exhaustion does not claim success. Searching raw database/WAL/IndexedDB fixture bytes does not reveal test names, chat text or secrets. Verify what metadata remains unencrypted and document it.

### 04 — `feat(identity): add least-privilege signer and consent adapters`

**Build:** Web NIP-07 with feature detection, NIP-46 remote signing for Linux and supported web use, and an Android NIP-55 adapter. Persist scoped account/source/destination permissions. Validate signer responses and bind asynchronous callbacks to the account and request that initiated them.

**Consent:** Separate capture, decrypt-and-retain, media fetching, replication, background activity, and routing-metadata signing. Do not request message-publication permissions for a read-only viewer. Authenticate only to approved relays. Treat remembered decryption grants as broader access rather than a perfect per-chat cryptographic permission.

**Gate:** Use at least one real signer flow on each platform. Test denial, absent optional methods, locked signer, wrong account, unexpected response, timeout, revoked remembered grant and canceled deep link. Failure creates a resumable `needs_authorization` state; it never asks for the user's nsec as a silent workaround or opens repeated background dialogs.

### 05 — `feat(backup): collect validated public events and private envelopes`

**Build:** Relay connections, explicit source configuration, bounded discovery, NIP-42 authentication, history collection and raw event persistence. Add typed collection profiles for supported public history, NIP-04 incoming/outgoing messages, and NIP-17 self-addressed gift wraps.

**Protocol:** NIP-04 incoming uses kind 4 plus `#p` for the user; outgoing uses kind 4 authored by the user. NIP-17 uses kind 1059 addressed to the user, not the user's author key. Discover the user's DM relay list from kind 10050 when available, while preserving manual historical sources.

**Gate:** Real local relays return the expected authorized fixtures. Forged, unrelated and malformed events are rejected. Duplicate IDs are stored once with multiple provenance records. Missing outgoing self-wraps, denied access, unavailable sources and truncated history appear as limitations, not empty complete conversations. No real public relays are used for destructive tests.

### 06 — `feat(chats): verify decrypted messages and build conversation projections`

**Build:** Decode captured history through the signer, validate the NIP-17 wrapper/seal/rumor chain, deduplicate logical rumors independently from outer event IDs, and construct encrypted thread/message/profile records. Use the original message timestamp for presentation, not randomized wrapper time.

**Identity:** Use stable public keys and canonical participant sets. Local aliases override profile display labels; missing profiles show abbreviated public keys. A profile-name change never splits or merges conversations. Resolve names only from permitted sources and do not load remote avatars by default.

**Lifecycle:** Implement public/legacy deletion authority, NIP-59 recipient wrapper-deletion rules, NIP-17 inner-message deletion, expiration, and replaceable metadata according to the pinned specifications. Suppress deleted/expired content from normal display and restore; expose retention limits for existing snapshots. Unsupported control kinds remain explicit.

**Gate:** Two wraps of one rumor produce one logical message. Sender/rumor mismatch is rejected. Pending decryption, failed decryption and verified display are distinct. Same-name contacts remain separate; new participants create the appropriate new conversation. Official vectors and real signer fixtures agree.

### 07 — `feat(sync): add resumable live capture and durable work queues`

**Build:** Combine historical collection with live subscriptions without a history/live gap. Transactionally save accepted events before advancing checkpoints; persist downstream decode, replica and snapshot intents. Retry with backoff/jitter, bounded concurrency, cancellation and per-account fairness.

**Catch-up:** Use per-relay/filter coverage, overlap and deduplication. NIP-17 wraps can be backdated by two days; a configurable seven-day overlap is a reasonable starting policy, not a completeness proof. Track randomized outer time separately from actual ingestion time. Handle saturated same-timestamp pages explicitly and use NIP-77 reconciliation where supported. An EOSE is not global completeness.

**Gate:** Publish from a new contact while running: capture it automatically, then add its named conversation after authorized decryption. Disconnect, publish while offline, reconnect and recover retained events. Crash between storage/queue transitions without losing work. Repeated remounts, tabs or retries do not create duplicate jobs/messages. Full queues and pauses are visible.

### 08 — `feat(archive): add encrypted portable snapshots and validated import`

**Build:** Implement the versioned encrypted-ZIP format, full original-envelope export, optional readable conversations, profile mappings, coverage reports and secure import staging. Use native and browser streaming/batching so large archives do not become one giant React object or IPC string.

**Snapshots:** Set a configurable cadence and debounce while unlocked; export immediately on explicit request. Publish a snapshot only after encryption/finalization succeeds. Track the exact checkpoint represented. Browser downloaded files are immutable snapshots unless a supported file permission was separately granted.

**Gate:** Export on web, import on Linux and Android; repeat in the other directions. Test wrong password, corruption, truncated/final-tag failures, invalid event signatures, filename collisions, malicious paths, duplicate entries, decompression bombs, excessive KDF parameters, interrupted output and insufficient disk space. The previous usable snapshot survives failure. No names are exposed in encrypted outer filenames.

### 09 — `feat(viewer): add offline read-only chat browsing and local search`

**Build:** Conversations sidebar/list, participant labels, chronological bubbles, date separators, local search, reply navigation, attachment placeholders, readable backup status and accessible responsive layouts. Virtualize large histories. Show protocol and decryption status unobtrusively where they matter.

**Offline:** Open the vault or an imported snapshot, unlock and browse already-materialized messages without contacting relays or the signer. Undecoded ciphertext remains clearly locked; a remote signer is not available offline just because the vault password is known.

**Privacy:** No message sending, read receipts, typing indicators, remote fonts, automatic link previews, remote avatar loads or telemetry. Render untrusted text safely. Any external link or attachment download is deliberate. Clear visible plaintext and local search state on lock.

**Gate:** Block all networking and confirm names, cached messages and saved media remain usable. Opening a conversation causes zero network calls. Test hostile HTML/SVG/text fixtures, keyboard navigation, narrow Android screens, large histories and changing contact labels. Searching does not create plaintext persistent indexes.

### 10 — `feat(recovery): add consent-aware replication and independent verification`

**Build:** Explicit destination policies, durable per-event/per-relay queues, a restore preview, lifecycle exclusions and independently obtained read-back results. An acknowledged upload is not verified recovery. Read actual destination bodies using a fresh network request, validate them and record observation time.

**Privacy:** Public supported events may go to selected public destinations. Private history is never handled by a generic public fan-out. For NIP-17, restore only the original wraps addressed to the current account and use its explicitly approved DM-inbox policy; separately confirm any kind-10050 update. Do not rewrite gifts for other participants, publish inner rumors, or claim ownership of an original random wrapper key. Legacy private data requires a restricted, expressly chosen destination and disclosure of metadata risks.

**Gate:** Stop the source, restore from retained storage to an empty compatible relay, then verify independently. Prove read-back after a lost ACK and failure when ACKs lie. Distinguish rejected, not verified, superseded, deleted, expired, protected and inaccessible events. Repeating restore is safe. Do not silently change kind-10002/10050 or delete old relays on timeout.

### 11 — `feat(media): add opt-in attachment preservation and safe offline previews`

**Build:** Media policy controls, size/disk quotas, explicit domain permission, resumable downloads, hashes, encrypted local objects and export/import integration. Parse supported NIP-17 file-message metadata through the verified message layer; keep attachment decryption keys inside protected storage.

**Network:** Validate schemes, redirects, resolved addresses and user-approved private/local exceptions. Keep TLS verification. Never silently route downloads through a centralized proxy to work around browser CORS. Record CORS-denied, unavailable, too-large, unsupported encryption and not-selected separately.

**Gate:** A supported image or file remains usable offline after its host disappears. Missing bytes are not called backed up because their URL exists. Invalid hashes/tags fail safely. Test redirect-to-private-address, oversized files, malicious active content, resumed-download consistency and path collisions. Preview only safe supported content; never execute imported files.

### 12 — `feat(web): finish PWA persistence and permission-aware snapshot export`

**Build:** Cache only versioned application assets for offline startup; persist encrypted user data through the vault adapter rather than service-worker response caches. Add storage persistence requests, usage/quota warnings, multi-tab leadership, reconnect/catch-up and browser capability detection.

**Files:** Provide standard download/import everywhere in the declared browser matrix. Optional selected-directory write-back must check capability and current permission. Snapshot files and vault state have separate timestamps; the UI must not imply an already-downloaded archive updates automatically.

**Gate:** Run real browser tests with the WASM engine, not mocked Tauri calls. Refresh/reopen, denied persistence, revoked file access, storage eviction, concurrent tabs and offline launch are covered. Foreground/background suspension recovers retained relay data when the app resumes. Documentation explicitly states closed-browser synchronization is not guaranteed.

### 13 — `feat(linux): add background engine lifecycle and durable snapshot scheduling`

**Build:** Optional keep-running behavior independent of the main window, a pause/status/quit control, opt-in autostart, sleep/wake recovery, single-instance coordination and filesystem snapshot retention. Reuse the same headless Rust runtime; avoid an unnecessary second service and second database.

**Lock:** Offer strict pause or the deliberately enabled encrypted capture-only spool. No decrypted archive generation while required vault keys are unavailable. Show capture and readable-snapshot freshness separately. AUTH-required relays pause when signer approval cannot be obtained.

**Gate:** Close the window while the configured process remains running, send a new fixture, reopen and find it persisted. Quit the actual process and confirm catch-up rather than fictitious continued activity. Test no-tray environments, Fedora and another supported distribution, sleep/resume, key-store unavailable, disk full and clean upgrade. Document packaging and runtime dependencies per tested build.

### 14 — `feat(android): add OS-scheduled catch-up and native archive access`

**Build:** A Tauri Kotlin plugin with WorkManager jobs, network/battery preferences, NIP-55 remembered-grant handling and Android document-tree import/export through the Storage Access Framework. Invoke the shared Rust runtime through JNI without requiring a live WebView or Activity. Use unique jobs and bounded batches with explicit cancellation.

**Schedule:** Suggested default is periodic catch-up rather than a perpetual foreground service. Choose a user-visible interval at or above WorkManager's minimum and label it approximate. Foreground large backups, when implemented, use the correct service type, user-visible notification, timeout handling and cancellation—not an invented exemption.

**Lock and privacy:** Respect device/keystore state and external signer grants. A worker unable to authenticate or access its approved capture configuration exits with an actionable state. Exclude private vaults and signer secrets from unintended platform cloud backup; never expose them through Android logs or URI query callbacks.

**Gate:** Test an actual device plus emulator: app foreground, screen off, process recreation without a WebView, doze, airplane mode, signer permission revocation, export permission revocation, user cancel and force-stop. Capture can be delayed; never claim continuous force-stopped execution. Archives open correctly after transfer to web/Linux. The UI must not claim background decryption when only raw capture succeeded.

### 15 — `test(release): prove cross-platform recovery and publish reproducible builds`

**Build:** Complete the three-platform fixture suite and the video runbook, accessibility pass, dependency/license review, release signing configuration and reproducible packaging. Add checksums and a compatibility/support matrix. Do not commit signing keys or actual user backups. Update AGENTS.md and MEMORY with implemented, verified and unverified items.

**Recovery drill:** Use separate-storage relays A, B and C; a sender outside the app-under-test; known original event IDs; incoming/outgoing NIP-04 and NIP-17 fixtures; duplicated wraps; one newly discovered contact; and an optional attachment. Capture to disk and replicate to B, stop A, restart the app, browse retained chats offline, restore eligible originals to C and read them from C without local-cache substitution. Keep A offline throughout.

**Gate:** Release web assets, a tested Linux package and a signed installable Android artifact only with their observed test results. Compare raw-event, logical-message, decoded-message and media counts separately. Test wrong password, rejected relay writes and unavailable signer visibly. A working Linux demonstration is not evidence Android background jobs passed.

## E. Commit discipline and AI-agent execution contract

For every milestone:

1. Read the current branch, AGENTS.md, relevant ADR and code; preserve unrelated work.
2. State the invariant and expected test before modifying code.
3. Implement only that milestone's coherent slice; add its tests alongside the feature.
4. Run the relevant native, WASM, frontend and integration checks. Record exact commands and results, including skipped/blocked checks.
5. Update durable memory and compatibility/status documentation.
6. Review the diff for secrets, misleading claims, unintended network calls and unrelated upgrades.
7. Create a commit only when explicitly authorized; do not push, release or mutate remote settings without permission.

These script names are proposed, not present in the inspected starter. Add them before referring to them as usable:

```text
npm run typecheck
npm run lint
npm run test:unit
npm run test:web
npm run build:web
npm run test:recovery
```

Rust checks should target the established workspace and actual package names. Add native formatting, Clippy, unit tests and WASM browser tests. Package Linux and Android with the repository-pinned Tauri CLI and documented platform prerequisites.

Use small change branches/PRs or one feature branch with reviewable milestones. Do not let parallel agents independently redesign the archive schema, signer contract, database or shared engine. Parallel work is reasonable only after those interfaces are stable.

## F. Demonstration storyboard

**Scene 1 — Permission:** Connect a test account using an external signer. Select relay A, enable readable encrypted backup and explicitly choose replica B. Show the selected permissions.

**Scene 2 — Real initial data:** Publish fixtures using a separate client/test producer. Show known incoming/outgoing messages and stable participant identities. Capture counters are calculated from actual events.

**Scene 3 — Automatic update:** Send a new message from a new contact. Show automatic capture, authorized decoding, conversation creation, replica processing and a completed archive checkpoint as separate stages.

**Scene 4 — Actual outage:** Stop relay A. Verify the connection fails. Restart NostrVault to remove the possibility that only RAM preserved the history.

**Scene 5 — Local recovery:** Disable networking entirely. Unlock the local vault and show named conversations and saved media. Explain undecoded/missing attachments honestly.

**Scene 6 — Portable recovery:** Transfer an encrypted snapshot to a clean browser profile or second device. Show wrong-password rejection, then unlock with the correct password. No Nostr identity secret is in the archive.

**Scene 7 — Relay restore:** Re-enable networking only for destination C, approve the restore plan and verify eligible originals from C. Keep A offline. Private replicas use the correct inbox policy, not arbitrary public fan-out.

**Scene 8 — Negative result:** Have C reject one fixture and show a partial report. The app does not turn a known failure green.

The video demonstrates this fixture set and supported protocols. It does not establish completeness of an arbitrary account, everlasting relay storage, protocol-wide migration, or anonymity.

## G. Release definition

The release is ready only when a user can create/unlock a vault, connect a supported signer, grant limited permissions, back up supported chats from selected relays, capture new messages while the platform permits execution, export/import a protected archive, read materialized conversations offline and restore eligible originals with timestamped read-back evidence.

Privacy gates: no identity secret in exports; no plaintext names/messages in persistent indexes/logs; no hidden media fetching; no private data copied through public fan-out; no silent plaintext fallback; controlled retention; and understandable lock/authorization behavior.

Platform gates: real web engine; real Linux packaging/lifecycle test; real Android signer, file and worker test. Mark failures or untested platforms rather than claiming 100% support.

Deadline discipline: a verified narrow end-to-end release is more valuable than unfinished broad support. First complete a NIP-17 text-chat capture -> protected archive -> offline viewer -> tested recovery path. Delay optional attachment conveniences and additional protocols before weakening security, faking background support or declaring unrun tests passed. Keep the 15-milestone roadmap even if submission ships a clearly labeled subset.

## H. Primary sources and grounding

Project inputs:
- Attached `AGENTS.md — NostrVault`, especially architecture ownership, private-history exclusions, lifecycle rules and independent read-back verification.
- Repository manifests inspected: `ANAMASGARD/NostrVault/package.json`, `src-tauri/Cargo.toml`; the fetched root `AGENTS.md` was empty. The attached guidance is therefore a separate document to incorporate, not assumed committed content.

Protocol references:
- Nostr NIP-01: https://github.com/nostr-protocol/nips/blob/master/01.md
- NIP-04 (legacy): https://github.com/nostr-protocol/nips/blob/master/04.md
- NIP-07 browser signer: https://github.com/nostr-protocol/nips/blob/master/07.md
- NIP-09 deletion requests: https://github.com/nostr-protocol/nips/blob/master/09.md
- NIP-17 private messages: https://github.com/nostr-protocol/nips/blob/master/17.md
- NIP-40 expiration: https://github.com/nostr-protocol/nips/blob/master/40.md
- NIP-42 relay authentication: https://github.com/nostr-protocol/nips/blob/master/42.md
- NIP-46 remote signer: https://github.com/nostr-protocol/nips/blob/master/46.md
- NIP-55 Android signer: https://github.com/nostr-protocol/nips/blob/master/55.md
- NIP-59 gift wraps: https://github.com/nostr-protocol/nips/blob/master/59.md
- NIP-65 relay metadata: https://github.com/nostr-protocol/nips/blob/master/65.md
- NIP-70 protected events: https://github.com/nostr-protocol/nips/blob/master/70.md
- NIP-77 reconciliation: https://github.com/nostr-protocol/nips/blob/master/77.md

Platform and implementation references:
- Rust nostr crate, including WASM support and feature flags: https://docs.rs/nostr/latest/nostr/
- Tauri Vite configuration: https://v2.tauri.app/start/frontend/vite/
- Tauri mobile plugins and Rust/JNI calls without an active WebView: https://v2.tauri.app/develop/plugins/develop-mobile/
- Tauri system tray: https://v2.tauri.app/learn/system-tray/
- Tauri Android distribution: https://v2.tauri.app/distribute/google-play/
- Tauri security capabilities: https://v2.tauri.app/security/capabilities/
- Tauri CSP: https://v2.tauri.app/security/csp/
- Android periodic work: https://developer.android.com/reference/androidx/work/PeriodicWorkRequest
- Android data-sync foreground service limits: https://developer.android.com/develop/background-work/services/fgs/timeout
- Android Storage Access Framework: https://developer.android.com/training/data-storage/shared/documents-files
- Android Keystore: https://developer.android.com/privacy-and-security/keystore
- Browser offline/background execution: https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Offline_and_background_operation
- Browser storage quota/eviction: https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria
- age format: https://age-encryption.org/v1
- age Rust library: https://docs.rs/age/latest/age/

Use pinned versions and protocol fixtures in implementation. These sources establish platform/protocol behavior; they do not prove NostrVault already implements the plan.
