# Security contract and initial threat model

Status: foundation controls under verification; not a security audit.

Protect Nostr identity/session credentials, vault keys, plaintext conversations, participant metadata, relay grants, and archive passwords. Untrusted inputs include relay replies, event text, URLs, archives, imported projections, and signer callbacks. Assume a malicious relay/archive author and interrupted writes; do not claim protection from a compromised running device or perfect JavaScript memory erasure.

## Current foundation

The bundled local UI has one custom permission: read compiled platform/version. The unused opener plugin and default core permission bundle are removed. Commands are listed in Tauri's app manifest so the custom ACL applies. Production CSP permits bundled scripts/styles and IPC only, blocks inline execution, frames, objects, and form submissions. Development CSP separately permits local Vite HMR. No account, relay, analytics, remote font, or attachment requests exist.

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
