# Chromium signer replacement research

Research checkpoint: 2026-10-05, within the authorized 03:23:32–03:53:32 UTC
investigation window. This note records source/artifact inspection, not a passed
runtime compatibility test. The prior nos2x 2.5.2 test reported decrypted synthetic
plaintext in the page console; that artifact is unsuitable for private decryption
under this milestone's privacy acceptance criteria. Third-party signers must not
be patched or have their logging suppressed to manufacture a pass.

## Candidate handed off for testing

**Alby 3.15.0** is available as an upstream production Chromium ZIP. The
[official release](https://github.com/getAlby/lightning-browser-extension/releases/tag/v3.15.0)
and [tag API](https://api.github.com/repos/getAlby/lightning-browser-extension/git/ref/tags/v3.15.0)
identify source commit `f3f2b2074382c47902abe6626628c5ac1a0deb76`.

- Artifact: `alby-chrome-v3.15.0.zip`, downloaded from the
  [official release asset](https://github.com/getAlby/lightning-browser-extension/releases/download/v3.15.0/alby-chrome-v3.15.0.zip).
- SHA256 calculated from downloaded bytes:
  `5cb551060b5ea0b83b3c4a9ce326b2ee2a2f63ae664696be414f7df054cc5cfe`.
  This matches the GitHub release API asset digest.
- Actual embedded manifest: version `3.15.0`, manifest version 3, minimum Chromium
  version 88, background service worker `js/background.bundle.js`.
- The archive contains 134 entries. It was inspected for unsafe archive paths and
  extracted unchanged into a temporary directory for isolated interoperability
  testing. Neither the archive nor its extracted application is committed.
- Trust limit: this ZIP has no CRX signature verification. Its hash and official
  HTTPS distribution establish the inspected artifact pin, not independent
  publisher-signature verification or source-to-binary reproducibility. A signed
  source commit does not sign the ZIP.

## Privacy inspection before runtime testing

The inspected [NIP-07 provider](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/providers/nostr/index.ts)
exposes both decryption APIs. Its
[page response bridge](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/providers/postMessage.ts)
and [content bridge](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/content-script/nostr.js)
forward successful results without a plaintext console sink.

The [NIP-04 handler](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/background-script/actions/nostr/decryptOrPrompt.ts)
and [NIP-44 handler](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/background-script/actions/nostr/nip44DecryptOrPrompt.ts)
return successful plaintext without logging or persisting it. Remembered permission
records are distinct from decrypted results. They do log decryption failures, so
runtime error-path and log scans remain necessary.

The [background dispatcher](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/background-script/index.ts)
logs full requests/responses only in development mode. Its production
[build command](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/package.json)
sets `NODE_ENV=production`. Inspection of the downloaded production background
bundle found the development `Background onMessage` and action-response log paths
absent. Action-name routing logs and decryption-error logging remain. This is
specific source/bundle evidence, not proof that every execution path is private.

## Isolated fixture setup and acceptance

Alby's regular onboarding is wallet-oriented. A test harness can provision a
throwaway profile through the extension's own internal setup/account/key commands;
the [account handler](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/background-script/actions/accounts/add.ts)
encrypts its configuration and the
[key-import handler](https://github.com/getAlby/lightning-browser-extension/blob/f3f2b2074382c47902abe6626628c5ac1a0deb76/src/extension/background-script/actions/nostr/setPrivateKey.ts)
encrypts the fixture identity. Such setup must remain in the disposable signer
profile, never in NostrVault's product flow. NostrVault never imports an identity
secret. Do not connect a wallet backend or use production funds/accounts.

Use actual signer approval prompts with per-request permission rather than broad
presets. Require public-key confirmation and synthetic NIP-04/NIP-44 operations,
then scan page/service-worker logs and durable browser backing files with positive
controls. Do not persist decrypted probe bodies as test evidence. Record setup
method and exact artifact separately from runtime outcomes.

The latest inspected nos2x-fox
[release](https://github.com/diegogurpegui/nos2x-fox/releases/tag/v1.21.0) offers a
Firefox XPI, not a Chromium artifact. Renaming or adapting it is not an unmodified
Chromium signer test. Alby was therefore handed off as the viable replacement.

If runtime approval/privacy evidence cannot pass before the authorized deadline,
Chromium private decryption remains explicitly unsupported for this MVP. Public
identity and the native signer paths continue; no fabricated private-capability
pass or extension patch is an acceptable fallback.

## Shortened investigation outcome

The maintainer asked to stop waiting for the full 30-minute budget and use the
smallest verified path. One local Alby fixture onboarding attempt did not complete
(the extension page closed during setup). No Alby plaintext-log/storage privacy
pass is claimed. Browser-extension private decryption is disabled in the candidate;
Linux NIP46 remains the verified synthetic private-message path. The artifact was
not patched and the nos2x privacy failure was not suppressed.
