# NostrVault

Your Nostr history should survive your relay.

NostrVault is a local-first backup and recovery application. Linux can capture approved relay history into an encrypted vault and restore eligible kind-1 notes with a separate read-back. Later slices add conversation projections, catch-up checkpoints, a portable `.nvarchive` envelope, offline search, attachment admission rules, an app-shell manifest, and engine/schedule contracts. See [implementation notes](MEMORY/memory.md) for what was actually exercised.

This is not a complete history product, a release, or proof of closed-app sync. M04 signer acceptance is still incomplete. Browser-extension private decryption stays disabled. The Linux recovery GUI script has not passed. Android WorkManager is specified, not registered. ARM64 runtime, physical devices, and the full signer matrix are unverified.

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
