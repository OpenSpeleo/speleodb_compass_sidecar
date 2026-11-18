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
            "actions/setup-node",
            "node-version:",
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
        "uploadUpdaterJson: false",
        "uploadUpdaterSignatures: false",
        "updaterJsonPreferNsis: false",
        "releaseDraft: true",
        "tagName: ${{ github.ref_name }}",
        "--target aarch64-apple-darwin",
        "windows-latest",
        "TAURI_SIGNING_PRIVATE_KEY:",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD:",
        "SENTRY_DSN_SPELEODB_COMPASS:",
        "APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}",
    ] {
        assert!(
            workflow.contains(required),
            "missing release contract: {required}"
        );
    }
    assert!(!workflow.contains("includeUpdaterJson:"));
    assert!(!workflow.contains("./src-tauri -> target"));
    assert!(!workflow.contains("APPLE_SIGNING_IDENTITY: \"-\""));
    assert!(!workflow.contains("secrets.APPLE_PASSWORD"));
}

fn assert_in_order(text: &str, markers: &[&str]) {
    let mut remaining = text;
    for marker in markers {
        let index = remaining
            .find(marker)
            .unwrap_or_else(|| panic!("missing or out-of-order release step: {marker}"));
        remaining = &remaining[index + marker.len()..];
    }
}

#[test]
fn release_workers_share_one_draft_and_aggregate_only_after_verification() {
    let workflow = publish_workflow();
    let prepare = named_step(&workflow, "Find or create the single draft");
    for required in [
        "scripts/release/project_version.py",
        "test \"$RELEASE_TAG\" = \"v$app_version\"",
        "gh api --paginate",
        "Multiple releases for tag",
        "Release must remain a draft",
        "--verify-tag",
        "--draft --title \"SpeleoDB Compass Sidecar $app_version\"",
        "release_id=$release_id",
    ] {
        assert!(
            prepare.contains(required),
            "missing draft guard: {required}"
        );
    }
    assert!(workflow.contains("releaseId: ${{ needs.prepare-release.outputs.release_id }}"));
    assert!(workflow.contains("needs: [prepare-release, release]"));
    let guard = named_step(&workflow, "Verify the draft before platform uploads");
    assert!(guard.contains(".draft == true and .tag_name == $tag"));
    let aggregate = named_step(
        &workflow,
        "Validate and publish the complete updater manifest",
    );
    for required in [
        "latest-macos-arm64.json",
        "latest-windows-x86_64.json",
        "release_metadata.py aggregate",
        ".draft == true and .tag_name == $tag",
        "gh release upload",
    ] {
        assert!(
            aggregate.contains(required),
            "missing aggregation guard: {required}"
        );
    }
    assert!(!workflow.contains("latest-linux"));
    assert_in_order(
        &workflow,
        &[
            "- name: Verify the draft before platform uploads",
            "- uses: tauri-apps/tauri-action@v1",
            "- name: Verify updater artifacts",
            "- name: Smoke test updater with the built release executable",
            "- name: Upload stapled macOS DMG",
            "- name: Generate platform updater metadata",
            "- name: Upload platform updater metadata for aggregation",
            "  aggregate-release:",
        ],
    );
}

#[test]
fn macos_release_imports_developer_id_and_staples_before_replacing_the_dmg() {
    let workflow = publish_workflow();
    assert!(workflow.contains("environment: ${{ matrix.environment }}"));
    assert!(workflow.contains("environment: macos-signing"));
    let credentials = named_step(&workflow, "Prepare Apple notarization credentials");
    for required in [
        "if: runner.os == 'macOS'",
        "umask 077",
        "secrets.APPLE_SIGNING_IDENTITY",
        "secrets.APPLE_API_KEY",
        "secrets.APPLE_API_ISSUER",
        "secrets.APPLE_API_PRIVATE_KEY",
        "APPLE_API_KEY_PATH=%s",
    ] {
        assert!(
            credentials.contains(required),
            "missing Apple credential contract: {required}"
        );
    }
    let certificate = named_step(&workflow, "Import Apple signing certificate");
    assert!(certificate.contains("apple-actions/import-codesign-certs@v7"));
    assert!(certificate.contains("secrets.APPLE_CERTIFICATE }}"));
    assert!(certificate.contains("secrets.APPLE_CERTIFICATE_PASSWORD"));
    let notarize = named_step(&workflow, "Notarize and staple macOS DMG");
    assert!(notarize.contains("\"${#dmgs[@]}\" -ne 1"));
    assert_in_order(
        notarize,
        &[
            "notarytool submit",
            "--wait --timeout 30m",
            "stapler staple",
        ],
    );
    let cleanup = named_step(&workflow, "Remove Apple notarization private key");
    assert!(cleanup.contains("if: always() && runner.os == 'macOS'"));
    assert!(cleanup.contains("rm -f \"${RUNNER_TEMP}/speleodb-compass-sidecar-apple-api-key.p8\""));
    assert_in_order(
        &workflow,
        &[
            "- name: Prepare Apple notarization credentials",
            "- name: Import Apple signing certificate",
            "- uses: tauri-apps/tauri-action@v1",
            "- name: Notarize and staple macOS DMG",
            "- name: Remove Apple notarization private key",
            "- name: Verify signed macOS package",
            "- name: Upload stapled macOS DMG",
        ],
    );
}

#[test]
fn windows_release_signs_during_bundling_using_the_environment_oidc_identity() {
    let workflow = publish_workflow();
    for required in [
        "environment: windows-signing",
        "id-token: write",
        "ARTIFACT_SIGNING_MODULE_VERSION: \"0.1.20\"",
        "secrets.AZURE_CLIENT_ID",
        "secrets.AZURE_TENANT_ID",
        "secrets.AZURE_SUBSCRIPTION_ID",
        "secrets.AZURE_SIGNING_ENDPOINT",
        "secrets.AZURE_SIGNING_ACCOUNT",
        "secrets.AZURE_SIGNING_PROFILE",
        "format('--config \"{0}\" {1}', steps.windows-signing.outputs.config,",
    ] {
        assert!(
            workflow.contains(required),
            "missing Windows signing contract: {required}"
        );
    }
    let prepare = named_step(&workflow, "Prepare Windows signing command");
    for required in [
        "if: runner.os == 'Windows'",
        "Install-Module -Name ArtifactSigning",
        "-RequiredVersion $env:ARTIFACT_SIGNING_MODULE_VERSION",
        "scripts/release/windows-sign.ps1",
        "signCommand = @{",
        "'-NoProfile', '-NonInteractive', '-File', $signer, '%1'",
    ] {
        assert!(
            prepare.contains(required),
            "missing signing callback setup: {required}"
        );
    }
    assert_in_order(
        &workflow,
        &[
            "- name: Cache Windows signing tools",
            "- name: Prepare Windows signing command",
            "- name: Log in to Azure for Windows signing",
            "uses: azure/login@v3",
            "- uses: tauri-apps/tauri-action@v1",
        ],
    );
}

#[test]
fn platform_checks_follow_the_cargo_target_output_paths() {
    let workflow = publish_workflow();
    assert!(workflow.contains("RELEASE_ROOT: ${{ matrix.release-root }}"));
    assert!(workflow.contains("release-root: target/aarch64-apple-darwin/release"));
    assert!(workflow.contains("release-root: target/release"));
    for name in [
        "Verify signed macOS package",
        "Verify updater artifacts",
        "Smoke test updater with the built release executable",
        "Generate platform updater metadata",
    ] {
        assert!(
            named_step(&workflow, name).contains("--release-root \"$RELEASE_ROOT\""),
            "{name} must use the matrix's Cargo target directory"
        );
    }
    for name in ["Notarize and staple macOS DMG", "Upload stapled macOS DMG"] {
        assert!(named_step(&workflow, name).contains("\"$RELEASE_ROOT\"/bundle/dmg/*.dmg"));
    }
}

#[test]
fn ci_skips_dependabot_prs_before_allocating_runners() {
    let workflow = workflow("ci.yml").replace("\r\n", "\n");
    for newline in ["\n", "\r\n"] {
        assert_dependabot_gating(&workflow.replace('\n', newline));
    }
}

fn assert_dependabot_gating(workflow: &str) {
    // Windows checkouts use CRLF; inspect the same job boundaries on every host.
    let workflow = workflow.replace("\r\n", "\n");
    let jobs = workflow
        .split("\njobs:\n")
        .nth(1)
        .expect("CI must define jobs");
    let names: Vec<_> = jobs
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':'))
        .collect();
    assert_eq!(
        names,
        ["  lint:", "  test:", "  trigger-release:"],
        "review Dependabot gating when adding CI jobs"
    );
    let (lint, downstream) = jobs.split_once("\n  test:\n").unwrap();
    assert!(
        lint.contains("    if: github.event.pull_request.user.login != 'dependabot[bot]'\n"),
        "gate by PR author so human updates and reruns cannot bypass the skip"
    );
    let (test, release) = downstream.split_once("\n  trigger-release:\n").unwrap();
    assert!(test.contains("    needs: [lint]\n"));
    assert!(release.contains("    needs: [test]\n"));
    for job in [test, release] {
        assert!(!job.contains("always()"));
        assert!(!job.contains("!cancelled()"));
    }
    assert!(workflow.contains("  push:\n    branches: [master]\n"));
    assert!(workflow.contains("  pull_request:\n    branches: [master]\n"));
    assert!(
        !workflow.contains("github.actor"),
        "gate PRs, not master push actors"
    );
}

#[test]
fn ci_gates_releases_on_native_and_frontend_checks() {
    let workflow = workflow("ci.yml");
    assert!(workflow.contains("platform: [windows-latest, macos-latest]"));
    assert!(named_step(&workflow, "Run native tests").contains("cargo test --workspace --locked"));
    assert!(named_step(&workflow, "Run frontend tests").contains("bun run test:ui"));
    assert!(
        named_step(&workflow, "Install browser engines")
            .contains("bunx --bun --no-install playwright install chromium webkit")
    );
    assert!(
        named_step(&workflow, "Build native application")
            .contains("bun run tauri build --no-bundle")
    );
    assert!(named_step(&workflow, "Require API credentials for trusted pushes").contains("exit 1"));
    let release = workflow.split("  trigger-release:").nth(1).unwrap();
    assert!(release.contains("needs: [test]"));
    assert!(release.contains("gh workflow run publish.yml --ref \"$TAG\""));
    assert!(
        named_step(&workflow, "Test release metadata and artifact verification")
            .contains("python -B -m unittest discover -s scripts/release")
    );
    let signing = named_step(&workflow, "Test Windows signing callback");
    assert!(signing.contains("if: runner.os == 'Windows'"));
    assert!(signing.contains("scripts/release/test_windows_sign.ps1"));
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
    assert_eq!(package["engines"]["bun"], bun_version.trim());
    assert_eq!(package["scripts"]["test"], "vitest run");
    assert!(
        package.get("version").is_none(),
        "Cargo remains the only app version source"
    );
}

#[test]
fn frontend_tools_and_formatter_hooks_use_bun_without_node_bootstrap() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let bunfig: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("app/bunfig.toml")).unwrap()).unwrap();
    assert_eq!(bunfig["run"]["bun"].as_bool(), Some(true));

    let hooks = fs::read_to_string(root.join(".pre-commit-config.yaml")).unwrap();
    let prettier = hooks
        .split("- id: prettier")
        .nth(1)
        .unwrap()
        .split("- repo:")
        .next()
        .unwrap();
    assert!(prettier.contains("entry: bun run --bun prettier --write --ignore-unknown"));
    assert!(prettier.contains("language: bun"));
    assert!(prettier.contains("language_version: system"));

    let browser_workflow =
        fs::read_to_string(root.join("app/.github/workflows/playwright.yml")).unwrap();
    assert!(browser_workflow.contains("bun-version-file: app/.bun-version"));
    assert!(!browser_workflow.contains("actions/setup-node"));
    assert!(!browser_workflow.contains("node-version:"));
    assert!(
        named_step(&browser_workflow, "Install Playwright Browsers")
            .contains("bunx --bun --no-install playwright install --with-deps")
    );
    assert!(
        named_step(&browser_workflow, "Run Playwright tests")
            .contains("bunx --bun --no-install playwright test")
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
