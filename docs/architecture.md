# Architecture decision: Rust-owned engine across three hosts

Status: milestone 03 adds production encrypted storage and vault sessions to the milestone 02 foundations. Signer, relay, conversation, archive-product, and recovery services remain planned. Platform verification is recorded separately in MEMORY/memory.md.

## Planned product direction

React owns presentation. A portable Rust core owns event validation, lifecycle/privacy policy, job transitions, archive contracts, and recovery evidence. Native hosts use Rust/SQLite/filesystem adapters behind narrow Tauri IPC. The web uses a Worker/WASM host with browser WebSocket, IndexedDB, and file adapters. Android will use a Kotlin integration and JNI to the same headless native runtime. Do not create a second backup database or policy engine in React.

Deep modules are event policy, transactional storage, conversation projection, archive handling, and recovery. Add seams only when a real host or testing distinction needs one. Maintained Nostr and encryption libraries must pass the native/WASM interoperability spike before adopting their APIs/format. Preserve signed field values and bound parsing, memory, queues, concurrency, retries, and cancellation.

The browser signer lives on the main page. The Worker sends typed requests through a broker that binds request ID, account, method, and vault-session generation. Lock, logout, cancellation, and account changes invalidate late replies. No identity secret fallback; no concurrent prompt storm. Native signer and relay authentication grants remain operation-specific.

Durable engine state is authoritative. Persist events before checkpoints and downstream intent before claiming queued work. Acknowledgement and independent destination read-back are separate results. Imported conversation projections remain archive-derived until locally validated through an authorized signer against original ciphertext.

## Implemented foundation

The root Cargo workspace contains the Tauri application and three crates. `vault-core` validates bounded Nostr fixtures and owns Argon2id/XChaCha20-Poly1305 operations and structured contracts. It has no platform dependencies or host randomness. `vault-native` supplies OS entropy, transactional SQLite, and an independent JNI library. `vault-wasm` supplies secure browser entropy and narrow wasm-bindgen exports. Both adapters compile the same small age compatibility module; Rust owns cryptographic policy. The Worker supplies IndexedDB transactions, not a second cryptographic engine.

The foundation operation validates a public signed fixture, runs shared adversarial vectors, wraps a random record key, encrypts fixture bytes, persists them, closes the store/runtime, reopens, and authenticates the recovered bytes. SQLite uses a new connection; the browser terminates Worker A and creates Worker B. Both adapters test atomic replacement, missing records, deletion, and rollback after an injected failure on the second write. Proof stores are isolated from future real vaults. The fixed fixture password is public and offers no secrecy for these public fixtures.

Core input limits are 64 KiB per event, 1 MiB payload, and 16 records per batch. The 1 KiB control envelope contains version, request ID, operation, and declared lengths; actual lengths are checked. Browser bytes use transferable buffers. Serialized encrypted proof records have an explicit larger bound to accommodate numeric JSON encoding; this bounded fixture encoding is not the future archive/media transport.

Local-vault compatibility uses Argon2id v19 with 8 MiB, t=1, p=1, and 32-byte output. XChaCha20-Poly1305 authenticates version, context, account, and record identity. These are non-production proof parameters. M03 production storage uses the separate format and parameter policy described below; proof stores are never imported.

Archive compatibility uses age 0.12.1 recipient and passphrase APIs over an opaque static ZIP fixture. Encryption finishes the stream; decryption must reach authenticated EOF. The independent fixtures come from Go age; native and WASM exchange fresh ciphertext in both directions. No ZIP writer, extraction, or importer is implemented. scrypt log2(N)=16 is the proof write factor; imports reject factors above 18 through the library before the requested KDF. With r=8/p=1, principal memory is about 64 MiB/256 MiB plus overhead. Constructors may calibrate locally. The optional age `web-sys` timer uses Window, not a Worker; the Worker instead uses explicit factors. No manual scrypt or age parser is introduced.

Native commands perform blocking work off the UI thread, use host-selected app-data paths, and allow only one active operation. Headless Kotlin/JNI calls need no Activity, WebView, or Tauri initialization. Native cancellation stops the caller waiting and rejects late results; already-running cryptography can finish. Browser cancellation terminates the Worker. Neither is durable job cancellation or rollback.

A single root lockfile preserves existing locked package versions. Explicit package gates inspect core dependency boundaries on native/WASM, build actual hosts, and retain CSP/ACL tests. The old numeric probe was removed after replacement browser, Linux, and Android platform checks passed; its load-failure behavior is covered by the real foundation Worker.

Web output is isolated in `dist-web/`; Tauri output remains `dist/`. This prevents a browser build from overwriting native assets while platform checks are being prepared. Tauri sets `TAURI_ENV_PLATFORM`, and Vite removes native IPC imports entirely from web assets.

## Accepted dependency proof

Pinned candidates compiled in their actual host paths: nostr 0.45.5, age 0.12.1, rusqlite 0.40.2, wasm-bindgen 0.2.129, JNI 0.22.4, Argon2 0.6.0, and chacha20poly1305 0.11.0. Cargo metadata reports MIT for nostr/rusqlite and MIT OR Apache-2.0 for the other direct candidates. This dependency review does not select a license for NostrVault.

Core disables default features on Nostr, Argon2, and AEAD, enabling only std, alloc, and key zeroization as declared in its manifest. Native SQLite uses bundled SQLite; JNI is Android-only. Both age adapters disable default features and the optional Window timing feature. Browser getrandom enables its secure JavaScript host adapter. The workspace also retains transitive older JNI/AEAD versions required by existing dependencies; these are not alternative application crypto implementations.

## Production vault (milestone 03)

The new `vault-core::vault` module owns production format, password policy,
wrapping, HKDF/HMAC account isolation, authenticated records, and session state.
Foundation functions and their namespaces remain isolated public-fixture proofs.
`vault-native::vault` owns the SQLite connection and OS lock; Tauri and the
headless JNI entry point select the same application-data directory. A bounded
native executor refuses overlapping protected operations. The browser Worker
holds a Web Lock and uses the same Rust session through `VaultSession`; the
IndexedDB adapter implements transactions, never cryptographic policy.

React renders the runtime status and transient password forms. Vault creation
commits encrypted setup together with the wrapper. Setup contains only completed
local protection/loss-acknowledgment choices; no account is invented. Native
password changes and browser changes both end the session. Lock invalidates
responses immediately, drains native work or terminates the Worker, then releases
ownership. The browser waits for ownership release before replacement-Worker
reopening because termination does not release a Web Lock synchronously.

Generic bounded account-scoped records, encrypted migration staging, and native
sealed-ingress APIs are internal building blocks. No public fixture-message UI,
background collection, signer, relay, archive product, or convenience unlock is
provided. See [storage format and limits](vault-storage.md).
