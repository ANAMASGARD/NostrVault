//! Android-only Tauri lifecycle: keep the Tao event loop alive across Activity
//! teardown and recreate the main WebView when a new Activity resumes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const MAIN_LABEL: &str = "main";

/// Tauri can retain a window handle after Android destroys the Activity WebView.
static MAIN_WEBVIEW_ATTACHED: AtomicBool = AtomicBool::new(false);

fn ensure_main_webview(app: &AppHandle) {
    if MAIN_WEBVIEW_ATTACHED.load(Ordering::Acquire) {
        return;
    }
    if app.get_webview_window(MAIN_LABEL).is_some() {
        if let Some(window) = app.get_webview_window(MAIN_LABEL) {
            let _ = window.destroy();
        }
    }
    match WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::default())
        .title("nostrvault")
        .build()
    {
        Ok(_) => MAIN_WEBVIEW_ATTACHED.store(true, Ordering::Release),
        Err(error) => log::error!("failed to recreate main webview after resume: {error}"),
    }
}

pub fn on_event(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            api.prevent_exit();
            if code.is_none() {
                detach_main_webview(app);
            }
        }
        RunEvent::Ready => {
            if app.get_webview_window(MAIN_LABEL).is_some() {
                MAIN_WEBVIEW_ATTACHED.store(true, Ordering::Release);
            }
        }
        RunEvent::Resumed => {
            ensure_main_webview(app);
        }
        RunEvent::WindowEvent {
            event: WindowEvent::Resumed | WindowEvent::Focused(true),
            ..
        } => {
            ensure_main_webview(app);
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } if label == MAIN_LABEL => {
            detach_main_webview(app);
            secure_lock_vault(app);
        }
        _ => {}
    }
}

fn detach_main_webview(app: &AppHandle) {
    MAIN_WEBVIEW_ATTACHED.store(false, Ordering::Release);
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.destroy();
    }
}

/// Bare-builder lifecycle handler (no vault lock); used by the minimal reproducer feature.
#[cfg(feature = "android-lifecycle-minimal")]
pub fn on_event_minimal(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            api.prevent_exit();
            if code.is_none() {
                detach_main_webview(app);
            }
        }
        RunEvent::Ready => {
            if app.get_webview_window(MAIN_LABEL).is_some() {
                MAIN_WEBVIEW_ATTACHED.store(true, Ordering::Release);
            }
        }
        RunEvent::Resumed => {
            ensure_main_webview(app);
        }
        RunEvent::WindowEvent {
            event: WindowEvent::Resumed | WindowEvent::Focused(true),
            ..
        } => {
            ensure_main_webview(app);
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } if label == MAIN_LABEL => {
            detach_main_webview(app);
        }
        _ => {}
    }
}

fn secure_lock_vault(app: &AppHandle) {
    let Some(runtime) = app.try_state::<OnceLock<Arc<vault_native::vault::Runtime>>>() else {
        return;
    };
    let Some(host) = runtime.get() else {
        return;
    };
    if let Err(error) = host.secure_lock() {
        log::error!("vault secure lock on window destroy failed: {error:?}");
    }
}
