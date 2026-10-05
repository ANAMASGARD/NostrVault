# Linux public-note recovery preview

This submission-day branch is a deliberately bounded vertical slice, not completion of M04 or the 15-milestone roadmap. Android's native Activity teardown crash remains unresolved and PR #4 remains draft. No new browser/Android signer support is claimed.

## What the Linux UI does

1. Create/unlock the existing encrypted vault.
2. Connect with the existing remote signer and explicitly confirm the account.
3. Enter and approve a source relay; press **Back up now**.
4. Read the retained public notes after the source and signer stop, including after an application restart. Unlocking and offline reading do not contact either service.
5. Enter a second destination and explicitly approve publication. **Restore and independently verify** sends eligible original signed notes, then opens a new connection to query and validate their IDs, bodies and signatures.

The UI separates transmitted, acknowledged, rejected and independently verified counts. An acknowledgement cannot become verification. Verification time and destination are saved encrypted. Unanswered verification remains unknown. A failed restore can already have transmitted events; cancellation is not rollback.

## Explicit limits

- Public text notes (kind 1) only, plus same-author kind-5 deletion evidence. No profile/contact/relay-metadata migration, private chats, archives, attachments or remote media retrieval.
- Manual capture and manual connection checks. There is no automatic polling, background execution, outage-triggered migration or advertised-relay change.
- One backed-up account per vault in this preview. A confirmed different account cannot overwrite the saved account's snapshot.
- At most 255 events in a source response; reaching 256 rejects the saturated result and preserves the existing backup. Up to 256 retained notes, 1,024 minimal deletion targets, 16 KiB per event and a 512 KiB encrypted snapshot payload. UI pages contain at most 16 notes.
- EOSE only ends the selected relay response. There is no exhaustive pagination or global completeness claim. An offline source cannot reveal deletion requests that were never observed.
- Protected and expired notes are not forwarded. Observed valid same-author deletions remove bodies and leave suppression IDs. Expired bodies are pruned on access. Restore also queries destination deletion evidence before publication.
- Authentication-required history relays fail explicitly in this slice. The remote signer transport is separate from public-history relay authentication; no broad signing permission is requested.
- Only explicitly selected endpoints are contacted. Production endpoints require WSS and certificate validation. Loopback WS is allowed for the deliberately selected local demo. No discovered relay hints, redirects or remote media are followed.
- The existing encrypted vault format, KDF, ownership and transaction/revision checks are preserved. The new versioned record is `public-note-backup-v1` in the encrypted vault-level namespace; it contains the single subject and its bounded snapshot. Lock/account-generation changes invalidate pending work. Already-started ciphertext commits may finish atomically.

## Reproducible disposable acceptance

Prepare the already-pinned nak 0.21.0 Linux artifact separately. The acceptance script checks SHA-256 `7bf6d8d82a9e9cf9aca74a04449fb2235bc49624a73b3539cf7d6f95304837a1` and never downloads artifacts.

```sh
npm run tauri -- build --bundles deb,rpm -- --locked
NOSTRVAULT_NAK=/absolute/path/to/nak-v0.21.0-linux-amd64 npm run test:recovery:linux
```

The script uses the real packaged Linux app through WebDriver, disposable profiles, local relays and a synthetic external signer identity. The application receives no private key. It generates 12 public notes, one observed deletion, one expired note and one protected note; the expected retained/restored set is 11. It starts B empty, captures from A, stops A and the signer, restarts the actual app, reads offline, tests an authentication-required destination, then restores to B and independently queries B using nak. It compares exact signed fields and scans raw SQLite for fixture plaintext. It removes the disposable profiles and stops its processes.

Evidence is written to ignored `test-results/linux-recovery.json`. The public unit fixture in `tests/fixtures/public-note.json` is generated with nak 0.21.0 using the known test-only scalar 1, timestamp 100 and content `NV_PUBLIC_UNIT_FIXTURE`; it is never an application identity.

Deterministic negative tests include altered signatures, wrong authors, saturated batches, deletion-before-target, expiration/protection, encrypted reopen, revision conflict, lock invalidation, and a controlled relay that acknowledges without returning the event (both completed-empty and interrupted read-back).

## Demo wording

“This is a bounded backup of 11 fixture public notes from this source. The source and signer are now stopped. After restarting and unlocking NostrVault, those 11 notes are still readable. I explicitly restore them to an empty second relay. A fresh independent query returns and validates those exact 11 signed notes.”

Do not call this all Nostr history, continuous backup, permanent storage, private-chat compatibility or automatic migration.
