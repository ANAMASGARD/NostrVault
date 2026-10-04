# NostrVault

Your Nostr history should survive your relay.

NostrVault is being built as a local-first backup and recovery application for supported Nostr history and conversations on web/PWA, Linux, and Android. It is **not yet a working backup application**. The current milestone establishes platform builds and real native Rust and browser Worker/WASM runtime checks; vaults, signers, collection, offline chats, archives, and recovery are not implemented.

The approved roadmap is in [the existing 15-milestone plan](MEMORY/NostrVault-15-Commit-Plan.md). Product behavior, architecture, and security requirements are documented in [product contract](docs/product-contract.md), [architecture decision](docs/architecture.md), and [security contract](docs/security.md). Actual progress and limitations are in [implementation notes](MEMORY/memory.md).

## Development

Use Node 22.23.1, npm 11.8.0, and the pinned Rust toolchain. Install frontend dependencies with `npm ci`. Native builds require the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The app identifier remains `com.nostrvault.app`.

```sh
npm run dev          # Web development; does not pretend native IPC exists
npm run tauri dev    # Native development
npm run verify:web   # Formatting, strict types, lint, unit tests, build, browsers
npm run verify:linux # Formatting, types, lint, units, Rust checks, packages, native smoke
npm run verify:android
npm run verify:commit # All required automated platform gates; fails on a missing gate
```

See [verification instructions](docs/verification.md) for prerequisites, exact commands, platform evidence, and CI. Passing a browser preview is not native evidence. No release publication, deployment, or production credentials are part of these commands.
