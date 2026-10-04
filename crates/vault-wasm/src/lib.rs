//! Narrow browser adapter. All cryptographic and event policy remains in Rust.
use js_sys::Uint8Array;
use vault_core::{FoundationRequest, ProtectedRecord, RandomMaterial, MAX_EVENT, MAX_PAYLOAD};
use wasm_bindgen::prelude::*;

#[path = "../../shared/age_proof.rs"]
mod age_proof;

fn error(code: &str) -> JsValue {
    JsValue::from_str(code)
}
fn core_error(value: vault_core::FoundationError) -> JsValue {
    match serde_json::to_value(value.code) {
        Ok(serde_json::Value::String(code)) => error(&code),
        _ => error("crypto"),
    }
}
fn input(bytes: &Uint8Array, limit: usize) -> Result<Vec<u8>, JsValue> {
    if bytes.length() as usize > limit {
        return Err(error("limit"));
    }
    Ok(bytes.to_vec())
}
fn random<const N: usize>() -> Result<[u8; N], JsValue> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).map_err(|_| error("randomness"))?;
    Ok(bytes)
}

#[wasm_bindgen]
pub fn protect(
    request: &str,
    event: &Uint8Array,
    payload: &Uint8Array,
) -> Result<Vec<u8>, JsValue> {
    if request.len() > 1024 {
        return Err(error("limit"));
    }
    let event = input(event, MAX_EVENT)?;
    let payload = input(payload, MAX_PAYLOAD)?;
    let request: FoundationRequest =
        serde_json::from_str(request).map_err(|_| error("malformed"))?;
    let validated = request.validate(&event, &payload).map_err(core_error)?;
    let entropy = RandomMaterial {
        salt: random()?,
        data_key: random()?,
        wrap_nonce: random()?,
        record_nonce: random()?,
    };
    let protected = vault_core::protect_record(
        age_proof::PROOF_PASSWORD.as_bytes(),
        &payload,
        &validated.pubkey,
        &validated.id,
        &entropy,
    )
    .map_err(|_| error("crypto"))?;
    serde_json::to_vec(&protected).map_err(|_| error("crypto"))
}

#[wasm_bindgen]
pub fn recover(event: &Uint8Array, protected: &Uint8Array) -> Result<Vec<u8>, JsValue> {
    let event = input(event, MAX_EVENT)?;
    // JSON numeric ciphertext encoding is private to this bounded fixture proof.
    let protected = input(protected, MAX_PAYLOAD * 4 + 4096)?;
    let validated = vault_core::validate_event(&event).map_err(core_error)?;
    let protected: ProtectedRecord =
        serde_json::from_slice(&protected).map_err(|_| error("malformed"))?;
    vault_core::open_record(
        age_proof::PROOF_PASSWORD.as_bytes(),
        &protected,
        &validated.pubkey,
        &validated.id,
    )
    .map(|value| value.to_vec())
    .map_err(core_error)
}

#[wasm_bindgen]
pub fn age_encrypt(bytes: &Uint8Array) -> Result<Vec<u8>, JsValue> {
    age_proof::encrypt_passphrase(&input(bytes, MAX_PAYLOAD)?).map_err(error)
}
#[wasm_bindgen]
pub fn age_decrypt(bytes: &Uint8Array) -> Result<Vec<u8>, JsValue> {
    age_proof::decrypt_passphrase(&input(bytes, MAX_PAYLOAD + 65536)?).map_err(error)
}
#[wasm_bindgen]
pub fn age_encrypt_recipient(bytes: &Uint8Array, recipient: &str) -> Result<Vec<u8>, JsValue> {
    if recipient.len() > 128 {
        return Err(error("limit"));
    }
    age_proof::encrypt_recipient(&input(bytes, MAX_PAYLOAD)?, recipient).map_err(error)
}
#[wasm_bindgen]
pub fn age_decrypt_recipient(bytes: &Uint8Array, identity: &str) -> Result<Vec<u8>, JsValue> {
    if identity.len() > 128 {
        return Err(error("limit"));
    }
    age_proof::decrypt_recipient(&input(bytes, MAX_PAYLOAD + 65536)?, identity).map_err(error)
}

#[wasm_bindgen]
pub fn validated_event_id(event: &Uint8Array) -> Result<String, JsValue> {
    vault_core::validate_event(&input(event, MAX_EVENT)?)
        .map(|value| value.id)
        .map_err(core_error)
}

#[wasm_bindgen]
pub fn core_proof() -> Result<(), JsValue> {
    vault_core::run_core_proof().map_err(core_error)
}

#[wasm_bindgen]
pub fn archive_proof(payload: &Uint8Array) -> Result<(), JsValue> {
    age_proof::verify_reference_proof(&input(payload, MAX_PAYLOAD)?).map_err(error)
}
