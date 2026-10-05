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

struct BrowserEntropy;
impl vault_core::vault::Entropy for BrowserEntropy {
    fn fill(&mut self, bytes: &mut [u8]) -> vault_core::vault::Result<()> {
        getrandom::getrandom(bytes).map_err(|_| vault_core::vault::Error::Randomness)
    }
}
fn vault_error(value: vault_core::vault::Error) -> JsValue {
    let code = serde_json::to_string(&value).unwrap_or_else(|_| "\"crypto\"".into());
    error(code.trim_matches('"'))
}
fn vault_json<T: serde::Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(|_| error("malformed"))
}
#[wasm_bindgen]
pub struct VaultSession {
    session: vault_core::vault::Session,
}
#[wasm_bindgen]
impl VaultSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<VaultSession, JsValue> {
        Ok(Self {
            session: vault_core::vault::Session::new(&mut BrowserEntropy).map_err(vault_error)?,
        })
    }
    pub fn status(&self) -> Result<String, JsValue> {
        vault_json(&self.session.status())
    }
    pub fn prepare(&mut self, request: &str, snapshot: &str) -> Result<String, JsValue> {
        if request.len() > 16384 || snapshot.len() > 8192 {
            return Err(error("limit"));
        }
        let request = serde_json::from_str(request).map_err(|_| error("malformed"))?;
        let snapshot = serde_json::from_str(snapshot).map_err(|_| error("malformed"))?;
        self.session
            .prepare(&request, &snapshot, &mut BrowserEntropy)
            .map_err(vault_error)
            .and_then(|m| vault_json(&m))
    }
    pub fn identity_index(&self) -> Result<String, JsValue> {
        self.session.identity_index().map_err(vault_error)
    }
    pub fn identity_pointer(&mut self, record: &str) -> Result<String, JsValue> {
        if record.len() > 65536 {
            return Err(error("limit"));
        }
        let record: Option<vault_core::vault::Record> =
            serde_json::from_str(record).map_err(|_| error("malformed"))?;
        vault_json(
            &self
                .session
                .identity_pointer(record.as_ref())
                .map_err(vault_error)?,
        )
    }
    pub fn identity_restore(&mut self, record: &str) -> Result<(), JsValue> {
        if record.len() > 65536 {
            return Err(error("limit"));
        }
        self.session
            .identity_restore(&serde_json::from_str(record).map_err(|_| error("malformed"))?)
            .map_err(vault_error)
    }
    pub fn identity_prepare(
        &mut self,
        request: &str,
        revision: u32,
        now: f64,
    ) -> Result<String, JsValue> {
        if request.len() > vault_core::identity::WIRE_LIMIT || !now.is_finite() || now < 0.0 {
            return Err(error("limit"));
        }
        let request = serde_json::from_str(request).map_err(|_| error("malformed"))?;
        vault_json(
            &self
                .session
                .identity_prepare(request, revision, now as u64, &mut BrowserEntropy)
                .map_err(vault_error)?,
        )
    }
    pub fn identity_committed(&mut self, revision: u32) {
        self.session.identity_committed(revision);
    }
    pub fn lookup_setup(&self) -> Result<String, JsValue> {
        self.session.lookup_setup().map_err(vault_error)
    }
    pub fn finish_unlock(&mut self, record: &str) -> Result<(), JsValue> {
        if record.len() > 16384 {
            return Err(error("limit"));
        }
        let record = serde_json::from_str(record).map_err(|_| error("malformed"))?;
        self.session.finish_unlock(&record).map_err(vault_error)
    }
    pub fn committed(&mut self) -> Result<(), JsValue> {
        self.session.committed().map_err(vault_error)
    }
    pub fn lock(&mut self) {
        self.session.lock();
    }
}

/// Isolated public-fixture benchmark; never accepts or returns user material.
#[wasm_bindgen]
pub fn vault_kdf_probe() -> Result<(), JsValue> {
    let _key = vault_core::vault::derive("benchmark test only", &[7; 16]).map_err(vault_error)?;
    Ok(())
}

/// Public synthetic record vectors for the isolated platform conformance harness.
/// No fixture identity or password is accepted by the production session API.
#[wasm_bindgen]
pub fn vault_record_vectors() -> Result<String, JsValue> {
    use vault_core::vault::Keys;
    let keys =
        Keys::create("production vector test only", &mut BrowserEntropy).map_err(vault_error)?;
    let record = keys
        .seal(
            "fixture-account-a",
            "fixture-record",
            1,
            b"M03 PRIVATE MESSAGE MARKER",
            &mut BrowserEntropy,
        )
        .map_err(vault_error)?;
    if keys
        .open("fixture-account-b", "fixture-record", &record)
        .is_ok()
    {
        return Err(error("crypto"));
    }
    for index in [0, 24, record.bytes.len() - 1] {
        let mut tampered = record.clone();
        tampered.bytes[index] ^= 1;
        if keys
            .open("fixture-account-a", "fixture-record", &tampered)
            .is_ok()
        {
            return Err(error("crypto"));
        }
    }
    if keys
        .open("fixture-account-a", "fixture-record", &record)
        .map_err(vault_error)?
        .as_slice()
        != b"M03 PRIVATE MESSAGE MARKER"
    {
        return Err(error("crypto"));
    }
    vault_json(&serde_json::json!({"header":keys.header,"record":record}))
}
