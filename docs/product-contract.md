# Approved product contract

This describes intended behavior, not delivered features. Milestone 03 implements only local vault protection and resumable local setup. Account connection and backup remain unavailable. The existing 15-milestone plan remains authoritative.

## First-run experience (03–07)

Connect an existing account → protect the local vault → approve history sources → review permissions → Start automatic backup → Overview. Web connects a browser signer, Linux guides QR/connection-link pairing, and Android connects an installed signer; supported remote signing is an alternative. Show the returned account before accepting grants. Missing/incompatible signers, wrong accounts, denied permissions, and cancelled pairing have retry/help actions and public-history/archive alternatives. Never ask for an identity secret.

Back preserves nonsecret setup choices. Before a vault exists, sensitive signer credentials remain transient; afterward persist them encrypted. Reopening resumes the last valid step and requests vault unlock when necessary. Changing account invalidates outstanding requests and grants. Cancelling setup does not delete an existing vault. Starting is idempotent across repeated clicks, restarts, and component remounts.

Use the vault password for new encrypted archives unless the user explicitly chooses another. Explain that old archives retain their original password, and a signer cannot reset archive encryption. Offline readable retention is an explicit choice; attachments, replication, and native background capture are separate opt-ins.

Discovery contacts only disclosed, approved lookup relays, then separately approved history sources. Show general relay metadata separately from DM inbox metadata. A successful connection is not proof of readable history or accepted restores. Empty, incomplete, unavailable, and authentication-required operations have different explanations and actions.

## Interface (06–09)

Navigation: Overview, Conversations, Backups, Settings. Desktop uses persistent navigation and conversation list/detail; mobile uses bottom navigation and explicit back navigation. Advanced sections contain protocol details without hiding limitations.

Use system typography at a 16px base, a 4px spacing grid, system light/dark defaults, visible keyboard focus, at least 44px interactive targets, text labels, and reduced-motion support. Components must define loading, empty, retryable-error, disabled, and completed behavior. No remote fonts, automatic avatars/media, fake progress, or unwired controls.

Use Working, Waiting for approval, Paused, Needs attention, Ready offline consistently. Offline availability is independent of capture activity. Show captured, decoded, replicated, and archived progress separately. Local vault, completed portable file, user-confirmed independent copy, and relay read-back evidence have distinct labels and timestamps. Starting a browser download is not proof that the file was saved elsewhere.

## Trust and recovery (08–10, 15)

Imported readable history remains **archive-derived** until authorized signer validation against original encrypted messages establishes the claimed author and contents. A valid password, authentication tag, hash, or imported verification flag is not that evidence. Offline reading remains available with its provenance label. Mismatched projections fail later validation instead of silently gaining trust.

Private-message replication/restoration follows the approved recipient-inbox policy and required authentication. Never publish unsigned plaintext rumors or forward private events through public fan-out. Routing changes require separate authorized signatures. Source outage alone does not authorize them.

The recovery demonstration keeps A offline, reads the vault/archive with both networking and signer disabled, then separately re-enables only the connections and signer access required for approved restoration to C. Read-back uses a fresh request, validates returned originals, and records the time; acceptance is not permanent-storage proof.

## Platform and acceptance boundaries

| Platform | Active and unlocked                      | Away or locked                                                               |
| -------- | ---------------------------------------- | ---------------------------------------------------------------------------- |
| Web      | Approved capture/decode while executable | Browser suspension/closure may stop work; resume catches up                  |
| Linux    | Approved capture/decode                  | Opt-in running background process; actual exit stops work                    |
| Android  | Approved capture/decode                  | Approximate scheduled catch-up under OS constraints; no force-stop guarantee |

Strict lock pauses by default. Optional native sealed capture stores encrypted originals without updating readable conversations. Signer unavailability pauses work requiring it. Purge validly deleted/expired bodies from active storage/new exports and keep minimal suppression records; exported past copies cannot be recalled.

Milestone 06 must prove a real signer, approved source, captured/decrypted message, restart, and offline signer-free reading on each target. Benchmark 1,000-message real-signer history and record prompt behavior. Milestones 09/15 require at least three unfamiliar people to connect, back up, find, export, and reopen without developer coaching. Record observations and retest fixes; automated tests do not substitute for those results.
