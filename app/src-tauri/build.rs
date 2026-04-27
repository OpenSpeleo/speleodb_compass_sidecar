fn main() {
    // Rebuild when the Sentry DSN env var changes so option_env!() picks it up.
    println!("cargo:rerun-if-env-changed=SENTRY_DSN_SPELEODB_COMPASS");

    tauri_build::try_build(build_attributes()).expect("tauri-build failed");
}

#[cfg(not(windows))]
fn build_attributes() -> tauri_build::Attributes {
    tauri_build::Attributes::new()
}

#[cfg(windows)]
fn build_attributes() -> tauri_build::Attributes {
    // tauri-build's default Windows app manifest is attached only to
    // [[bin]] targets (via `cargo:rustc-link-arg-bins=...` inside
    // embed-resource), leaving test/example/bench exes without the
    // Common Controls v6 dependency declaration. Without that manifest
    // the Windows loader serves the ComCtl5 stub, which deliberately
    // omits v6 entry points and aborts the test binary at startup with
    // STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) the moment any
    // Tauri/dialog/updater symbol is in the import table.
    //
    // Suppress tauri-build's bin-only manifest here and embed our own
    // copy via embed_resource::compile_for_everything() below so that the
    // same manifest lands in every linked artifact, tests included.
    //
    // See: https://github.com/tauri-apps/tauri/issues/13419
    // See: docs/windows-test-manifest.md
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    embed_app_manifest_for_all_artifacts();
    attributes
}

#[cfg(windows)]
fn embed_app_manifest_for_all_artifacts() {
    // `compile_for_everything` emits `cargo:rustc-link-arg=...` (no `-bins`
    // suffix), so the resource is linked into bins, examples, benches, AND
    // unit/integration test binaries — the gap that tauri-build itself
    // leaves open. `manifest_required()` turns a missing RC toolchain into
    // a hard build failure rather than a silent skip, which is the correct
    // posture for a loader-critical manifest.
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    println!("cargo:rerun-if-changed=windows-app-manifest.rc");
    embed_resource::compile_for_everything("windows-app-manifest.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
