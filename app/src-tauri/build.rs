fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "bootstrap",
            "set_preferences",
            "complete_onboarding",
            "save_rifle",
            "save_scope",
            "save_bullet",
            "save_load",
            "save_profile",
            "delete_record",
            "set_active_profile",
            "solve",
            "range_card",
            "chart",
            "export_library",
            "import_library",
        ]));
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
