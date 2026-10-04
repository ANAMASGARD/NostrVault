//! Regenerate public test-only fixture. Never use this key as an identity.
use nostr::prelude::{EventId, Kind, PublicKey, Timestamp};
use secp256k1::{Keypair, Secp256k1, SecretKey};
fn main() {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[1; 32]).unwrap();
    let pair = Keypair::from_secret_key(&secp, &key);
    let pk = PublicKey::from(pair.x_only_public_key().0);
    let ts = Timestamp::from(1_700_000_000);
    let kind = if std::env::args().any(|arg| arg == "--out-of-scope") {
        Kind::Metadata
    } else {
        Kind::TextNote
    };
    let id = EventId::compute(
        &pk,
        &ts,
        &kind,
        &Default::default(),
        "NostrVault public test fixture",
    );
    let sig = secp.sign_schnorr_no_aux_rand(id.as_bytes(), &pair);
    println!(
        "{}",
        serde_json::json!({"id":id.to_hex(),"pubkey":pk.to_hex(),"created_at":1700000000,"kind":kind.as_u16(),"tags":[],"content":"NostrVault public test fixture","sig":sig.to_string()})
    );
}
