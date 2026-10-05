# M03 production storage decision

Status: M03 implementation. Actual platform checks and remaining limits are recorded in MEMORY/memory.md.

Production vaults are independent of the public M02 proof stores. One active
vault occupies the host-selected `vault/vault.sqlite` or browser database
`nostrvault-vault`. No path is accepted from presentation code. Format, schema,
record and wrapper versions are independent. Version 1 uses a random 256-bit
root, Argon2id v19 at exactly 65536 KiB/3 passes/4 lanes, a 16-byte salt and
32-byte output. Unsupported profiles are refused before allocation. One KDF
runs at a time in the owning runtime. Passwords are exact UTF-8, at most 1024
bytes, with at least 12 Unicode scalar values for new passwords.

XChaCha20-Poly1305 protects the root and records with fresh 24-byte nonces.
HKDF-SHA256 derives account record and lookup keys with distinct versioned
labels and vault/account context. HMAC-SHA256 produces opaque lookup keys.
Associated context uses JSON arrays of typed values, never concatenated strings.
Wrapper context authenticates format, vault ID, profile, salt and revision;
record context authenticates vault/account/lookup/record-format/revision.
Password changes replace only the wrapper, never the root or existing records.

Native SQLite uses DELETE journaling, FULL synchronization and memory temp
storage. Browser writes acknowledge transaction completion, with strict durability
requested; this is not a guarantee against every power loss. Sensitive data
enters stores only after encryption. Limits: 4 KiB header; 1 MiB plaintext
record; 16 records and 4 MiB plaintext per batch/page. Ciphertext adds a 24-byte
nonce and 16-byte tag; record versions/revisions are separate columns.

Creation commits the header and setup together. Revision checks and transactions
protect updates. Future schemas are rejected without writes. Migration fixtures
are explicitly earlier production schemas, not foundation databases. Encrypted
staging and an atomic active-generation switch preserve old usable data until
migration finalization. Never delete or recreate a vault after an error.

One session owns an OS file lock (native) or Web Lock (browser). Ownership covers
unlock through lock, including password change. Lock invalidates generations
before waiting for cleanup. Ciphertext transactions already committing may
finish; callers must reconcile durable state after cancellation. Password change
ends the session. Fresh processes and Workers start locked. No keys or plaintext
are broadcast between contexts. Strict pause is the default; device unlocking,
relay collection and background execution are not implemented by M03.

Visible metadata includes format and schema, random vault ID, KDF profile,
wrapper/record revisions, opaque lookup equality, counts, ciphertext sizes and
operational timing. No names, identities or plaintext search indexes are stored.
Zeroization limits retained Rust secrets but cannot promise erasure from browser
strings, OS memory, swap, screenshots or compromised devices. Authentication
does not establish freshness: old authenticated records or whole-store snapshots can be replayed by an attacker with storage access. Browser storage
can be cleared or evicted. Old external copies retain their old passwords; a
signer cannot recover a lost vault password.

## Storage and migration details

Storage schema 2 adds encrypted staging and native spool tables to the explicitly
labeled schema-1 migration fixture. Record/wrapper/vault formats remain 1. Native
migration callbacks handle one bounded encrypted record at a time under a SQLite
transaction. Browser payload migration stages bounded encrypted pages, verifies
same-key coverage, then replaces the active records within a single transaction.
An interrupted staging pass can be restarted; failed finalization preserves old
active records. Staging requires space for a second ciphertext copy plus journal
or transaction overhead; quota failures abort without dropping the active vault.

Records include an opaque account namespace, opaque lookup key, version,
revision, and nonce-prefixed ciphertext. Account paging uses the namespace and
bounded key cursor. Native blobs and browser records expose no plaintext indexes.
Browser record bytes use bounded numeric arrays in structured-clone storage;
this carries serialization overhead and is not a portable archive format.

New ingress batches use age X25519, a 1 MiB plaintext-envelope bound, at most
64 KiB ciphertext overhead, and 64 MiB/256 pending-batch quotas. Ciphertext digest
keys deduplicate exact appends; encrypted body-digest receipts deduplicate replay
and reject conflicting batch identities. The candidate and receipt merge and
spool removal share a transaction. No grants or recipient configuration are
persisted in plaintext. This is a synthetic-input primitive, not live backup.

Crypto dependencies remain pinned: Argon2 0.6.0 and XChaCha20Poly1305 0.11.0;
HKDF 0.12.4, HMAC 0.12.1 and SHA-256 0.10.9 were already present transitively and
are now explicit core dependencies. Native age remains 0.12.1. No custom crypto
primitive or dependency upgrade is introduced.
