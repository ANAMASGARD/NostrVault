# Security contract and initial threat model

Status: foundation and M03 vault controls; observed verification is recorded in MEMORY/memory.md. This is not a security audit.

Protect Nostr identity/session credentials, vault keys, plaintext conversations, participant metadata, relay grants, and archive passwords. Untrusted inputs include relay replies, event text, URLs, archives, imported projections, and signer callbacks. Assume a malicious relay/archive author and interrupted writes; do not claim protection from a compromised running device or perfect JavaScript memory erasure.

## Current foundation

The bundled local UI permits compiled runtime information, isolated foundation proofs, and the bounded vault command. The unused opener plugin and default core permission bundle are removed. Commands are listed in Tauri's app manifest so the custom ACL applies. Production CSP permits bundled scripts/styles and IPC only, blocks inline execution, frames, objects, and form submissions. Development CSP separately permits local Vite HMR. No account, relay, analytics, remote font, or attachment requests exist.

Unit checks inspect capability/CSP configuration. Linux WebDriver checks actual Rust IPC, denied unapproved window IPC, and blocked inline script execution. Browser tests capture page errors and unexpected network requests. These tests do not prove future integrations are secure.

## Required as features land

- Encryption: maintained AEAD/KDF, bounded parameters, random data keys, encrypted sensitive records and staging, atomic migration/finalization where supported, wrong-password/tamper failure, no silent plaintext fallback.
- Trust: archive integrity is independent of message authorship. Imported readable history retains archive-derived provenance until signer-assisted comparison with original encrypted messages succeeds.
- Publication: immutable originals, current lifecycle eligibility, selected destinations, recipient-inbox private routing, explicit AUTH and metadata-signing grants. No permission bypass or public private-data fan-out.
- Native/IPC: validate requests and paths again in Rust; capabilities do not sandbox custom Rust. Add only necessary commands/scopes; no shell construction from input or unrestricted filesystem access.
- Content/network: render untrusted text safely; deliberate safe-scheme external links; approved endpoints only; validate URLs, redirects, private-address exceptions, and TLS. No automatic media/discovery.
- Lifecycle: revoke outstanding work on lock/logout/account change; reject stale signer replies. Encrypted optional ingress cannot claim readable freshness. Never log message bodies, identity secrets, session tokens, or passwords.
- Platforms: exclude sensitive native data from unintended cloud backup; browser caches contain application assets only. Test production policies, not just a permissive development build.

See [Tauri capabilities](https://v2.tauri.app/security/capabilities/) and [CSP](https://v2.tauri.app/security/csp/). Protocol behavior is pinned and verified when its milestone lands.

## Production vault boundaries

Production KDF policy is fixed at Argon2id v19, 65536 KiB, three passes and four
lanes. Header parameters are checked before work. Explicit zeroizing KDF buffers
are necessary: the pinned library allocation helper does not itself wipe its
whole allocated workspace. Wrapping and record keys are Rust-owned; passwords
are never persisted. Errors expose categories, not underlying input or secret
material. Authentication failure is deliberately ambiguous between a wrong
password and altered protected data.

Native OS locks and browser Web Locks coordinate contexts for the entire
unlocked session. Database revision checks fence stale mutations. Ciphertext may
commit during lock; lock never claims rollback. Account namespace identifiers
and record lookup keys are keyed, opaque values. Their equality, counts, sizes,
versions and operational timing remain observable.

Native sealed ingress uses age recipient encryption with vault-protected private
keys and transient explicit grants. Decrypted batches enter an untrusted-candidate
namespace. Atomic encrypted receipts prevent duplicate merges; encryption does
not authenticate a sender. Strict pause remains default, with no collection UI.

Limits include browser eviction, imperfect memory erasure, whole-store rollback,
and unrecalled external copies. Fault-injection tests are not physical power-loss
proof. Fixture-file scans and logical IndexedDB scans have different coverage;
report accessible browser backing-file inspection separately.

## M04 candidate signer restrictions

No identity-secret fallback exists. Future capture, replication, attachment,
background and routing scopes remain inactive; empty scope denies. API presence
is distinct from approval. Decrypt capability probes need an explicit grant and
user action, use fresh disposable encrypted material, and contact no public relay.

nos2x 2.5.2 is incompatible with private chats: a disposable real run observed
its provider logging the decrypted probe result in the page console. Do not mask
that console output or label it a privacy pass. Browser-extension private decrypt
is currently disabled in Rust and UI. Alby 3.15.0 was researched but did not finish
runtime privacy acceptance within the shortened investigation; it is not approved.
Firefox extension private decryption is also unverified and disabled for now.

AUTH requests require a confirmed account, exact approved relay/challenge,
unchanged empty content and exactly two tags, age <=120 seconds and future skew
<=30 seconds. Verify canonical ID and signature after approval. The result is
consumable only as an AUTH message. Pending approvals expire after 120 seconds;
client pairing expires after 300 seconds. Lock/cancel/revocation invalidate late
results, and the main-page broker checks cancellation before additional prompts.

Current limitations: full malicious NIP46 transport and Android callback/recreation
acceptance is unfinished; signer-proposed relay switching and remote logout are
not implemented. They must not be advertised as successful revocation or adopted
silently. Real signers are separate trust boundaries; an API capability observation
is not an assurance that a third-party signer handles data privately.
