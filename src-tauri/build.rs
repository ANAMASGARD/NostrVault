fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "runtime_info",
            "foundation_proof",
            "vault_command",
            "identity_command",
            "identity_transport",
            "identity_packages",
        ]),
    ))
    .expect("failed to build Tauri application metadata");
}
