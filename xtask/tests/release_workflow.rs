use std::fs;
use std::path::Path;

fn workflow(name: &str) -> String {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be located directly below the repository root");
    let workflow_path = repository_root.join(".github/workflows").join(name);

    fs::read_to_string(&workflow_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", workflow_path.display()))
}

fn publish_workflow() -> String {
    workflow("publish.yml")
}

fn named_step<'a>(workflow: &'a str, name: &str) -> &'a str {
    let marker = format!("- name: {name}");
    let start = workflow
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let line_start = *offset;
            *offset += line.len();
            Some((line_start, line))
        })
        .find_map(|(offset, line)| (line.trim() == marker).then_some((offset, line)))
        .unwrap_or_else(|| panic!("workflow does not contain a `{name}` step"));
    let indentation = start.1.len() - start.1.trim_start().len();
    let remaining = &workflow[start.0..];
    let end = remaining
        .split_inclusive('\n')
        .skip(1)
        .scan(start.1.len(), |offset, line| {
            let line_start = *offset;
            *offset += line.len();
            Some((line_start, line))
        })
        .find_map(|(offset, line)| {
            let line_indentation = line.len() - line.trim_start().len();
            (line_indentation == indentation && line.trim_start().starts_with("- "))
                .then_some(offset)
        })
        .unwrap_or(remaining.len());

    &remaining[..end]
}

#[test]
fn named_step_handles_lf_and_crlf_line_endings() {
    let workflow = "jobs:\n  release:\n    steps:\n      - name: Install frontend dependencies\n        run: bun install --frozen-lockfile\n        working-directory: app\n      - name: Publish\n        run: publish\n";
    for newline in ["\n", "\r\n"] {
        let workflow = workflow.replace('\n', newline);
        let step = named_step(&workflow, "Install frontend dependencies");
        assert!(step.contains("bun install --frozen-lockfile"));
        assert!(step.contains("working-directory: app"));
        assert!(!step.contains("- name: Publish"));
    }
}

#[test]
fn workflows_install_the_pinned_frontend_from_its_lockfile() {
    for name in ["ci.yml", "publish.yml"] {
        let workflow = workflow(name);
        assert!(workflow.contains("bun-version-file: app/.bun-version"));
        let install = named_step(&workflow, "Install frontend dependencies");
        assert!(install.contains("bun install --frozen-lockfile"));
        assert!(install.contains("working-directory: app"));
        assert!(workflow.contains("hashFiles('app/.bun-version', 'app/bun.lock')"));
        for obsolete in [
            "trunk",
            "wasm-pack",
            "wasm-bindgen",
            "wasm32-unknown-unknown",
            "geckodriver",
        ] {
            assert!(
                !workflow.contains(obsolete),
                "{name} still requires {obsolete}"
            );
        }
    }
}

#[test]
fn publish_preserves_packaging_and_updater_contracts() {
    let workflow = publish_workflow();
    for required in [
        "projectPath: app",
        "tauriScript: bun run tauri",
        "SIDECAR_UI_PROFILE: release",
        "uploadUpdaterJson: true",
        "releaseDraft: true",
        "tagName: v__VERSION__",
        "--target aarch64-apple-darwin",
        "windows-latest",
        "TAURI_SIGNING_PRIVATE_KEY:",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD:",
        "SENTRY_DSN_SPELEODB_COMPASS:",
        "APPLE_SIGNING_IDENTITY: \"-\"",
    ] {
        assert!(
            workflow.contains(required),
            "missing release contract: {required}"
        );
    }
    assert!(!workflow.contains("includeUpdaterJson:"));
    assert!(!workflow.contains("./src-tauri -> target"));
}

#[test]
fn ci_gates_releases_on_native_and_frontend_checks() {
    let workflow = workflow("ci.yml");
    assert!(workflow.contains("platform: [windows-latest, macos-latest]"));
    assert!(named_step(&workflow, "Run native tests").contains("cargo test --workspace --locked"));
    assert!(named_step(&workflow, "Run frontend tests").contains("bun run test:ui"));
    assert!(
        named_step(&workflow, "Build native application")
            .contains("bun run tauri build --no-bundle")
    );
    assert!(named_step(&workflow, "Require API credentials for trusted pushes").contains("exit 1"));
    let release = workflow.split("  trigger-release:").nth(1).unwrap();
    assert!(release.contains("needs: [test]"));
    assert!(release.contains("gh workflow run publish.yml --ref \"$TAG\""));
}

#[test]
fn frontend_package_uses_the_shared_bun_pin_without_a_release_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let bun_version = fs::read_to_string(root.join("app/.bun-version")).unwrap();
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("app/package.json")).unwrap()).unwrap();
    assert_eq!(
        package["packageManager"],
        format!("bun@{}", bun_version.trim())
    );
    assert_eq!(package["private"], true);
    assert!(
        package.get("version").is_none(),
        "Cargo and Tauri remain the only app version sources"
    );
}

#[test]
fn bun_dependabot_preserves_floors_and_avoids_routine_lockfile_churn() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let config = fs::read_to_string(root.join(".github/dependabot.yml")).unwrap();
    let bun = config
        .split("- package-ecosystem: \"bun\"")
        .nth(1)
        .expect("Bun dependency maintenance must remain configured");
    for required in [
        "directory: \"/app\"",
        "versioning-strategy: \"widen\"",
        "dependency-type: \"direct\"",
        "dependency-name: \"*\"",
        "version-update:semver-minor",
        "version-update:semver-patch",
    ] {
        assert!(
            bun.contains(required),
            "missing Bun update policy: {required}"
        );
    }
    assert!(!bun.contains("version-update:semver-major"));
    assert!(
        !bun.contains("exclude-paths:"),
        "Bun locks must accompany range widening"
    );
    assert!(!bun.contains("open-pull-requests-limit: 0"));
}
