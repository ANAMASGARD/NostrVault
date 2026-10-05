use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::Manager;
use vault_core::{ErrorCode, FoundationError};

#[cfg(target_os = "android")]
mod android_lifecycle;

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

#[tauri::command]
async fn vault_command(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, OnceLock<Arc<vault_native::vault::Runtime>>>,
    request: String,
) -> Result<vault_core::vault::Status, vault_core::vault::Error> {
    let request = zeroize::Zeroizing::new(request);
    if request.len() > 16384 {
        return Err(vault_core::vault::Error::Limit);
    }
    let request: vault_core::vault::Request =
        serde_json::from_str(&request).map_err(|_| vault_core::vault::Error::Malformed)?;
    if runtime.get().is_none() {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| vault_core::vault::Error::Storage)?
            .join("vault");
        let host = Arc::new(vault_native::vault::Runtime::new(directory)?);
        let _ = runtime.set(host);
    }
    let runtime = Arc::clone(runtime.get().ok_or(vault_core::vault::Error::Storage)?);
    if matches!(request.operation, vault_core::vault::Operation::Lock) {
        runtime.invalidate();
    }
    tauri::async_runtime::spawn_blocking(move || runtime.execute(&request))
        .await
        .map_err(|_| vault_core::vault::Error::Storage)?
}

#[tauri::command]
async fn identity_command(
    runtime: tauri::State<'_, OnceLock<Arc<vault_native::vault::Runtime>>>,
    request: String,
) -> Result<vault_core::identity::Output, vault_core::vault::Error> {
    let request = zeroize::Zeroizing::new(request);
    if request.len() > vault_core::identity::WIRE_LIMIT {
        return Err(vault_core::vault::Error::Limit);
    }
    let request =
        serde_json::from_str(&request).map_err(|_| vault_core::vault::Error::Malformed)?;
    let runtime = Arc::clone(runtime.get().ok_or(vault_core::vault::Error::Locked)?);
    tauri::async_runtime::spawn_blocking(move || runtime.identity_execute(request))
        .await
        .map_err(|_| vault_core::vault::Error::Storage)?
}
#[tauri::command]
async fn identity_transport(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, OnceLock<Arc<vault_native::vault::Runtime>>>,
    binding: vault_core::identity::Binding,
    id: String,
) -> Result<serde_json::Value, vault_core::vault::Error> {
    let runtime = Arc::clone(runtime.get().ok_or(vault_core::vault::Error::Locked)?);
    let effect = runtime.identity_effect(&binding, &id)?;
    #[cfg(target_os = "android")]
    if effect.adapter == vault_core::identity::Adapter::Android {
        let plugin = app.state::<tauri::plugin::PluginHandle<tauri::Wry>>();
        let response = plugin
            .run_mobile_plugin_async("request", effect)
            .await
            .map_err(|_| vault_core::vault::Error::Cancelled)?;
        runtime.identity_effect(&binding, &id)?;
        return Ok(response);
    }
    let _ = (app, effect);
    vault_native::identity_transport::receive(runtime, binding, id).await
}
#[tauri::command]
async fn identity_packages(app: tauri::AppHandle) -> Result<Vec<String>, vault_core::vault::Error> {
    #[cfg(target_os = "android")]
    {
        return app
            .state::<tauri::plugin::PluginHandle<tauri::Wry>>()
            .run_mobile_plugin_async("packages", serde_json::json!({}))
            .await
            .map_err(|_| vault_core::vault::Error::Unsupported);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(vec![])
    }
}
#[tauri::command]
async fn backup_command(
    runtime: tauri::State<'_, OnceLock<Arc<vault_native::vault::Runtime>>>,
    request: String,
) -> Result<vault_native::backup::Output, vault_core::vault::Error> {
    if request.len() > 8192 {
        return Err(vault_core::vault::Error::Limit);
    }
    let request =
        serde_json::from_str(&request).map_err(|_| vault_core::vault::Error::Malformed)?;
    let runtime = Arc::clone(runtime.get().ok_or(vault_core::vault::Error::Locked)?);
    vault_native::backup::execute(runtime, request).await
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

#[cfg(all(target_os = "android", feature = "android-lifecycle-minimal"))]
fn run_android_minimal() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .build(tauri::generate_context!())?
        .run(|app, event| android_lifecycle::on_event_minimal(app, event));
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(all(target_os = "android", feature = "android-lifecycle-minimal"))]
    {
        if let Err(error) = run_android_minimal() {
            eprintln!("Application runtime failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(
        tauri::plugin::Builder::<tauri::Wry>::new("signer")
            .setup(|app, api| {
                let plugin = api.register_android_plugin("com.nostrvault.app", "SignerPlugin")?;
                app.manage(plugin);
                Ok(())
            })
            .build(),
    );
    let app = match builder
        .manage(OnceLock::<Arc<vault_native::vault::Runtime>>::new())
        .invoke_handler(tauri::generate_handler![
            runtime_info,
            foundation_proof,
            vault_command,
            identity_command,
            identity_transport,
            identity_packages,
            backup_command
        ])
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Application runtime failed: {error}");
            std::process::exit(1);
        }
    };
    app.run(|app, event| {
        #[cfg(target_os = "android")]
        android_lifecycle::on_event(app, event);
        #[cfg(not(target_os = "android"))]
        {
            let _ = (app, event);
        }
    });
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
