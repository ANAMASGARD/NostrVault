# Architecture decision: Rust-owned engine across three hosts

Status: approved direction; shared engine implementation begins in milestone 02.

React owns presentation. A portable Rust core owns event validation, lifecycle/privacy policy, job transitions, archive contracts, and recovery evidence. Native hosts use Rust/SQLite/filesystem adapters behind narrow Tauri IPC. The web uses a Worker/WASM host with browser WebSocket, IndexedDB, and file adapters. Android will use a Kotlin integration and JNI to the same headless native runtime. Do not create a second backup database or policy engine in React.

Deep modules are event policy, transactional storage, conversation projection, archive handling, and recovery. Add seams only when a real host or testing distinction needs one. Maintained Nostr and encryption libraries must pass the native/WASM interoperability spike before adopting their APIs/format. Preserve signed field values and bound parsing, memory, queues, concurrency, retries, and cancellation.

The browser signer lives on the main page. The Worker sends typed requests through a broker that binds request ID, account, method, and vault-session generation. Lock, logout, cancellation, and account changes invalidate late replies. No identity secret fallback; no concurrent prompt storm. Native signer and relay authentication grants remain operation-specific.

Durable engine state is authoritative. Persist events before checkpoints and downstream intent before claiming queued work. Acknowledgement and independent destination read-back are separate results. Imported conversation projections remain archive-derived until locally validated through an authorized signer against original ciphertext.

Milestone 01 contains a native host-health command and a dependency-free Rust WASM probe in `tools/runtime-probe`. The browser loads the compiled module inside a Worker, invokes its numeric probe through a validated request/result bridge, and displays the Rust result. Fetch, compilation, export, Worker and timeout failures reject the check; no TypeScript result fallback exists. Workers terminate after success/failure. This proves host execution, not a shared engine, storage, crypto, signers or recovery.

Web output is isolated in `dist-web/`; Tauri output remains `dist/`. This prevents a browser build from overwriting native assets while platform checks are being prepared. Tauri sets `TAURI_ENV_PLATFORM`, and Vite removes native IPC imports entirely from web assets.
