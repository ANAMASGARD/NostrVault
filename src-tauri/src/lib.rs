use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use vault_core::{ErrorCode, FoundationError};

static FOUNDATION_RUNNING: AtomicBool = AtomicBool::new(false);

struct FoundationGuard;
impl Drop for FoundationGuard {
    fn drop(&mut self) {
        FOUNDATION_RUNNING.store(false, Ordering::Release);
    }
}

#[tauri::command]
async fn foundation_proof(
    app: tauri::AppHandle,
    request: String,
) -> Result<serde_json::Value, FoundationError> {
    if request.len() > 1024 {
        return Err(FoundationError::new(ErrorCode::Limit));
    }
    if FOUNDATION_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(FoundationError::new(ErrorCode::Busy));
    }
    let guard = FoundationGuard;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| FoundationError::new(ErrorCode::Storage))?
        .join("m02-foundation");
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        std::fs::create_dir_all(&directory)
            .map_err(|_| FoundationError::new(ErrorCode::Storage))?;
        let response = vault_native::run_foundation(&request, &directory.join("proof.sqlite"));
        serde_json::from_str(&response).map_err(|_| FoundationError::new(ErrorCode::Malformed))
    })
    .await
    .map_err(|_| FoundationError::new(ErrorCode::Crypto))?
}

#[derive(Serialize)]
struct RuntimeInfo {
    platform: &'static str,
    version: &'static str,
}

#[tauri::command]
fn runtime_info() -> RuntimeInfo {
    RuntimeInfo {
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![runtime_info, foundation_proof])
        .run(tauri::generate_context!())
    {
        eprintln!("Application runtime failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_contract_uses_the_compiled_platform_and_version() {
        let value = serde_json::to_value(runtime_info()).unwrap();
        assert_eq!(value["platform"], std::env::consts::OS);
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}
