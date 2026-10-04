# Public age interoperability fixtures

These keys/passwords are public test material, never application identities.
The recipient and passphrase archives were generated independently with Go
`filippo.io/age v1.2.1`, using scrypt log2(N)=16. The plaintext is the static ZIP
at `crates/vault-core/fixtures/payload.zip` (see its fixture documentation).

Reproduce from this directory with:

```sh
go run . ../../vault-core/fixtures/payload.zip .
```

Encryption is randomized, so regenerated ciphertext hashes will differ.
Current SHA-256:

- payload ZIP: `be0ebbf0d9b3bad87fd8d619a1862b62cea8879e549e75b6d2ee03a82aafdcb9`
- reference-passphrase.age: `14d25bf27175ad2867b7e52671e130ce3ba41d3e687a7e0e29a9690468d773dc`
- reference-recipient.age: `d6be479be4575910fa70c059b21c3d7af16cc203a6b518a5ac6613e2c4f1e2c0`

Go is only needed to regenerate reference fixtures, not to build NostrVault.
