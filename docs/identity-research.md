# Milestone 04 signer research

Research and artifact inspection date: 2026-10-05. This records protocol evidence,
implementation decisions, and candidate interoperability artifacts. **Artifact
verification is not signer-flow verification: all four real signer flows remain
unverified at this research checkpoint.** Runtime results belong in
`MEMORY/memory.md` and the verification reports. Machine-readable artifact pins are
in `tests/signers/artifacts.json`; downloaded binaries are not repository inputs.

## Protocol boundaries

[NIP-07](https://github.com/nostr-protocol/nips/blob/master/07.md) exposes the signer
through `window.nostr`. Detect public-key access, event signing, NIP-04 decryption,
and NIP-44 decryption independently. The decrypt methods are optional. Method
presence establishes API availability, not authorization or remembered permission.
The browser page brokers extension calls; Rust owns account, consent, pending
request, and session-generation checks. Inspection of the M03 baseline found a
planned broker boundary in documentation, not an existing implementation.

[NIP-46](https://github.com/nostr-protocol/nips/blob/master/46.md) distinguishes the
remote transport signer key from the user's account key. Fetch the account key
after connecting and require user confirmation. Its encrypted request/response
transport uses kind 24133 and NIP-44. The client-initiated flow requires an
unpredictable secret and exact echo verification. A bunker token can contain a
single-use secret; there is no universal token-expiry field. Validate signed event
author, signature, recipient, request ID, expected operation, account, and vault
session generation before accepting results. Pairing relays are signer transport,
not consent to collect user history. Signer-proposed relay changes require the
same network-policy validation as initial pairing.

[NIP-55](https://github.com/nostr-protocol/nips/blob/master/55.md) supports native
Android Activity Result requests. Discover handlers with the `nostrsigner` manifest
query; bind subsequent requests to the selected package and confirmed
`current_user`. Supply an unpredictable `id` and reject mismatched, duplicate, or
late callbacks. `RESULT_OK` plus `rejected=true` denotes denial; cancelled/non-OK
results are distinct failures. M04 uses foreground Activity Results, not plaintext
URL/clipboard callbacks or background Content Resolver access. The spec's Content
Resolver text and example disagree about where query arguments reside; this
unused transport must not be inferred from that example.

## Least privilege and honest capability reporting

- NIP-46 and NIP-55 have no standardized exhaustive capability-discovery call.
  Keep optional capabilities unknown until supported by an explicit request or
  test; distinguish denial, unsupported methods, and temporary unavailability.
- A benign, explicitly approved decryption probe can encrypt a fresh unpredictable
  challenge to the confirmed account using maintained primitives. Do not request
  real history to test a capability. A returned plaintext alone is not signed
  evidence of authorship or a universal guarantee against a malicious signer.
- Keep local account-scoped grants separate from signer permissions. Optional
  grants begin disabled. Recording future replication, attachment retrieval, or
  background consent must not start those features in M04.
- Relay authentication is a narrow operation: require the approved account,
  relay, and challenge; constrain signing to kind 22242; verify the returned
  signature, canonical ID, and unchanged requested fields. Do not expose general
  arbitrary-event signing as a shortcut. See
  [NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md).
- NIP-07 and NIP-55 do not provide a portable permission-revocation API. Disconnect
  deletes local remembered credentials and invalidates callbacks; users may also
  revoke permissions inside their signer. NIP-46 `logout` is a courtesy: delete
  the local client key regardless of acknowledgement. Lock stops protected work
  and transport. Never request the user's identity secret as a fallback.

## Maintained cryptography and runtime boundary

The baseline pins `nostr` 0.45.5. Keep that version and enable only needed maintained
NIP-04/NIP-44/NIP-46 primitives. The selected design is a bounded Rust protocol and
policy state machine with thin platform transports. This does not implement
cryptography from scratch.

The inspected `nostr-connect` 0.45.2 crate accepts `nostr` 0.45, but caches the
account key and its connect request lacks current permission/metadata fields;
it also lacks the current `switch_relays` and `logout` methods. Its higher-level
client is therefore not adopted wholesale. See the published
[client source](https://docs.rs/crate/nostr-connect/0.45.2/source/src/client.rs) and
[pinned core protocol source](https://docs.rs/crate/nostr/0.45.5/source/src/nips/nip46.rs).
Context7 was queried for `/nostrdevkit/nostr`; its general examples were checked
against these concrete sources rather than treated as version-specific evidence.

## Verified candidate artifacts

The JSON pins contain full SHA256 values and source references. Verification used
the downloaded artifact bytes, not only displayed release versions.

| Candidate                  | Artifact inspection                                                                                                                                                                                         | Verification limit                                                                                                                              |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Chromium nos2x 2.5.2       | Actual Chrome Web Store CRX manifest reports 2.5.2; CRX3 RSA publisher identity matches extension `kpgefcfmnafjgpblomihpgmejjdanjjp`; all three CRX signatures verified                                     | Source reference `014493f9602d0a3826ef3eab2bdd4901ee315cce` is pinned separately; reproducible source-to-CRX correspondence was not established |
| Firefox nos2x-fox 1.21.0.1 | Release tag v1.21.0; embedded manifest reports 1.21.0.1; CMS signature verifies against Mozilla's add-on root; `.sf` manifest digest and all 51 entry SHA256 digests pass; no unlisted nonsignature entries | Offline CMS verification is not Firefox installation or runtime approval-flow evidence                                                          |
| Linux nak v0.21.0          | Download hash pinned; Go build metadata reports source revision `2437c91c46564099994029a0856dcc7e3b287d91`, Go 1.27.1, linux/amd64                                                                          | Embedded `vcs.modified=true`; no independent binary signature or reproducible source-to-binary match established                                |
| Android Amber 6.6.6        | Offline x86_64 APK versionName 6.6.6/versionCode 208, package `com.greenart7c3.nostrsigner`; manifest hash matches; APK v2/v3 signatures verify; signing certificate matches downloaded 6.5.1 APK           | GPG manifest has a mathematically valid signature from an expired key; no claim of current GPG-key validity or reproducible build               |

Primary sources: [nos2x source reference](https://github.com/fiatjaf/nos2x/tree/014493f9602d0a3826ef3eab2bdd4901ee315cce),
[Chrome Web Store entry](https://chromewebstore.google.com/detail/nos2x/kpgefcfmnafjgpblomihpgmejjdanjjp),
[nos2x-fox release](https://github.com/diegogurpegui/nos2x-fox/releases/tag/v1.21.0),
[Mozilla add-on root](https://github.com/mozilla-firefox/firefox/blob/main/security/manager/ssl/addons-public.pem),
[nak release](https://github.com/fiatjaf/nak/releases/tag/v0.21.0), and
[Amber release](https://github.com/greenart7c3/Amber/releases/tag/v6.6.6).

### Amber signature details

The official latest-release API returned v6.6.6, published
2026-09-28T11:22:07Z. Its tag resolves to
`fc2e0de5ddcc6f25ac1ae08f43a0073440a70355`. The actual APK has minSDK 26 and targetSDK 37. Android build-tools 36.0.0 `apksigner verify --verbose --print-certs` verifies
v2 and v3 signatures with certificate SHA256
`e8ab8c69333b68636dd46ce242408c79553a7fd9055d054d61daababada53bbf`.

The public key from [the developer's GitHub account](https://github.com/greenart7c3.gpg)
matches fingerprint `44F0AAEB77F373747E3D5444885822EED3A26A6D` published in
[upstream verification instructions](https://github.com/greenart7c3/Amber/blob/v6.6.6/VERIFY_RELEASES.md).
In an isolated temporary keyring, both 6.6.6 and 6.5.1 manifests produce `VALIDSIG`
and `EXPKEYSIG`: the fetched key expired at Unix 1755883803, before these
signatures. The recommended keys.openpgp.org endpoint reset the connection;
the alternative Ubuntu keyserver returned 404. No renewed key was obtained.
Observed APK signing continuity does not remove that GPG limitation.

## Interoperability execution still required

Use disposable signer profiles and test identities, never the user's real account.
Run actual browser extension approval, Linux pairing/session, and installed Android
signer Activity Result flows. Independently exercise controlled negative fixtures
for denial, missing capabilities, locked signer, timeout, wrong account, revocation,
invalid pairing, cancelled Android callback, and stale/duplicate/late results.
Fixtures do not substitute for upstream signer interoperability.

[nak's bunker source](https://github.com/fiatjaf/nak/blob/2437c91c46564099994029a0856dcc7e3b287d91/bunker.go)
prints pairing material and decrypted protocol requests/results by default. Verify
quiet-mode behavior before running tests and do not retain raw subprocess output.
Scan application logs/storage with positive controls. These artifact checks did
not install or run any signer and did not demonstrate any M04 acceptance flow.
