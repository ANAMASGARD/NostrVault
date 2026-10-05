#[path = "../../shared/age_proof.rs"]
pub mod age_proof;
pub mod spool;
pub mod storage;
pub mod vault;
use std::path::Path;
use vault_core::{
    ErrorCode, FoundationError, FoundationRequest, FoundationResponse, FoundationResult,
    RandomMaterial, EVENT_JSON, VERSION, ZIP_BYTES,
};

static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    }
}

fn random_material() -> vault_core::Result<RandomMaterial> {
    let mut random = RandomMaterial {
        salt: [0; 16],
        data_key: [0; 32],
        wrap_nonce: [0; 24],
        record_nonce: [0; 24],
    };
    for bytes in [
        &mut random.salt[..],
        &mut random.data_key[..],
        &mut random.wrap_nonce[..],
        &mut random.record_nonce[..],
    ] {
        getrandom::getrandom(bytes).map_err(|_| FoundationError::new(ErrorCode::Randomness))?;
    }
    Ok(random)
}
fn proof(request: &FoundationRequest, path: &Path) -> vault_core::Result<FoundationResult> {
    let event = request.validate(EVENT_JSON, ZIP_BYTES)?;
    vault_core::run_core_proof()?;
    age_proof::verify_reference_proof(ZIP_BYTES)
        .map_err(|_| FoundationError::new(ErrorCode::Crypto))?;
    let protected = vault_core::protect_record(
        age_proof::PROOF_PASSWORD.as_bytes(),
        ZIP_BYTES,
        &event.pubkey,
        &event.id,
        &random_material()?,
    )?;
    let encoded =
        serde_json::to_vec(&protected).map_err(|_| FoundationError::new(ErrorCode::Crypto))?;
    storage::conformance(path, &encoded)?;
    let mut store = storage::Store::open(path)?;
    store.write_batch(&[(&event.id, &encoded)])?;
    drop(store);
    let store = storage::Store::open(path)?;
    let stored = store
        .read(&event.id)?
        .ok_or_else(|| FoundationError::new(ErrorCode::Storage))?;
    let protected =
        serde_json::from_slice(&stored).map_err(|_| FoundationError::new(ErrorCode::Storage))?;
    let plaintext = vault_core::open_record(
        age_proof::PROOF_PASSWORD.as_bytes(),
        &protected,
        &event.pubkey,
        &event.id,
    )?;
    if plaintext.as_slice() != ZIP_BYTES {
        return Err(FoundationError::new(ErrorCode::Crypto));
    }
    let encrypted = age_proof::encrypt_passphrase(ZIP_BYTES)
        .map_err(|_| FoundationError::new(ErrorCode::Crypto))?;
    if age_proof::decrypt_passphrase(&encrypted)
        .map_err(|_| FoundationError::new(ErrorCode::Crypto))?
        != ZIP_BYTES
    {
        return Err(FoundationError::new(ErrorCode::Crypto));
    }
    Ok(FoundationResult {
        event_id: event.id,
        payload_bytes: ZIP_BYTES.len(),
        storage_reopened: true,
        crypto_verified: true,
    })
}
/// Paths are host-selected, never supplied by an untrusted frontend request.
pub fn run_foundation(request_json: &str, database_path: &Path) -> String {
    let request = if request_json.len() > 1024 {
        Err(FoundationError::new(ErrorCode::Limit))
    } else {
        serde_json::from_str::<FoundationRequest>(request_json)
            .map_err(|_| FoundationError::new(ErrorCode::Malformed))
    };
    let request_id = request
        .as_ref()
        .ok()
        .filter(|r| r.request_id.len() <= 64)
        .map(|r| r.request_id.clone())
        .unwrap_or_default();
    let outcome = request.and_then(|request| {
        if ACTIVE
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            return Err(FoundationError::new(ErrorCode::Busy));
        }
        let _guard = ActiveGuard;
        proof(&request, database_path)
    });
    let response = match outcome {
        Ok(result) => FoundationResponse {
            version: VERSION,
            request_id,
            result: Some(result),
            error: None,
        },
        Err(error) => FoundationResponse {
            version: VERSION,
            request_id,
            result: None,
            error: Some(error),
        },
    };
    serde_json::to_string(&response).unwrap_or_else(|_| {
        "{\"version\":1,\"requestId\":\"\",\"result\":null,\"error\":{\"code\":\"crypto\"}}".into()
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nostrvault_app_HeadlessFoundation_nativeRun<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _object: jni::objects::JObject<'caller>,
    request: jni::objects::JString<'caller>,
    path: jni::objects::JString<'caller>,
) -> jni::objects::JString<'caller> {
    let outcome = unowned_env.with_env(|env| -> std::result::Result<_, jni::errors::Error> {
        let length = env
            .call_method(&request, jni::jni_str!("length"), jni::jni_sig!("()I"), &[])?
            .i()?;
        if length > 1024 {
            return jni::objects::JString::from_str(
                env,
                "{\"version\":1,\"requestId\":\"\",\"result\":null,\"error\":{\"code\":\"limit\"}}",
            );
        }
        let request = request.try_to_string(env)?;
        let path = path.try_to_string(env)?;
        jni::objects::JString::from_str(env, run_foundation(&request, Path::new(&path)))
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_native_request_rejects_a_second_without_work() {
        assert!(!ACTIVE.swap(true, std::sync::atomic::Ordering::Acquire));
        let _guard = ActiveGuard;
        let request = serde_json::to_string(&FoundationRequest {
            version: VERSION,
            request_id: "second".into(),
            operation: "foundation_proof".into(),
            event_length: EVENT_JSON.len(),
            payload_length: ZIP_BYTES.len(),
        })
        .unwrap();
        let response: FoundationResponse = serde_json::from_str(&run_foundation(
            &request,
            Path::new("must-not-be-created.sqlite"),
        ))
        .unwrap();
        assert_eq!(response.error.unwrap().code, ErrorCode::Busy);
        assert!(response.result.is_none());
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nostrvault_app_HeadlessVault_nativeRun<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _object: jni::objects::JObject<'caller>,
    request: jni::objects::JString<'caller>,
    path: jni::objects::JString<'caller>,
) -> jni::objects::JString<'caller> {
    static HOST: std::sync::OnceLock<vault::Runtime> = std::sync::OnceLock::new();
    let outcome = unowned_env.with_env(|env| -> std::result::Result<_, jni::errors::Error> {
        let length = env
            .call_method(&request, jni::jni_str!("length"), jni::jni_sig!("()I"), &[])?
            .i()?;
        if length > 16384 {
            return jni::objects::JString::from_str(env, "{\"error\":\"limit\"}");
        }
        let request = zeroize::Zeroizing::new(request.try_to_string(env)?);
        let path = path.try_to_string(env)?;
        let result = (|| -> vault_core::vault::Result<_> {
            if HOST.get().is_none() {
                let host = vault::Runtime::new(std::path::PathBuf::from(path))?;
                let _ = HOST.set(host);
            }
            let request =
                serde_json::from_str(&request).map_err(|_| vault_core::vault::Error::Malformed)?;
            HOST.get()
                .ok_or(vault_core::vault::Error::Storage)?
                .execute(&request)
        })();
        let response = match result {
            Ok(status) => serde_json::json!({"status":status}),
            Err(error) => serde_json::json!({"error":error}),
        };
        jni::objects::JString::from_str(env, response.to_string())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nostrvault_app_HeadlessVault_nativeBenchmark<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _object: jni::objects::JObject<'caller>,
) -> jni::objects::JString<'caller> {
    let outcome = unowned_env.with_env(|env| -> std::result::Result<_, jni::errors::Error> {
        let mut times = Vec::new();
        for run in 0..6 {
            let start = std::time::Instant::now();
            if vault_core::vault::derive("benchmark test only", &[7; 16]).is_err() {
                return jni::objects::JString::from_str(env, "{\"error\":\"crypto\"}");
            }
            if run > 0 {
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        jni::objects::JString::from_str(
            env,
            serde_json::json!({"profile":"65536KiB/t3/p4","milliseconds":times,"memoryKiB":65536})
                .to_string(),
        )
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

pub mod identity_transport;
