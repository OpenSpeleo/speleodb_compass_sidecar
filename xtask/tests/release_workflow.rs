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
    let workflow = "\
jobs:
  release:
    steps:
      - name: Install trunk
        env:
          GITHUB_TOKEN: token
        run: cargo binstall trunk
      - name: Publish
        run: publish
";

    for newline in ["\n", "\r\n"] {
        let workflow = workflow.replace('\n', newline);
        let install_step = named_step(&workflow, "Install trunk");

        assert!(install_step.contains("GITHUB_TOKEN: token"));
        assert!(install_step.contains("cargo binstall trunk"));
        assert!(!install_step.contains("- name: Publish"));
    }
}

#[test]
fn install_trunk_step_is_authenticated_and_locked() {
    let workflow = publish_workflow();
    let install_step = named_step(&workflow, "Install trunk");

    assert!(
        install_step.contains("shell: bash"),
        "the cross-platform Trunk bootstrap must run its shell script with Bash"
    );
    assert!(
        install_step.contains("GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}"),
        "the Trunk bootstrap must authenticate cargo-binstall GitHub API requests"
    );
    assert!(
        install_step.contains("cargo binstall --no-confirm --force --locked trunk"),
        "the Trunk bootstrap must repair a missing binary despite stale Cargo install metadata"
    );
}

#[test]
fn publish_workflow_uses_current_repository_paths() {
    let workflow = publish_workflow();

    assert!(
        !workflow.contains("./src-tauri -> target"),
        "the removed top-level src-tauri cache workspace must not return"
    );
    assert!(
        workflow.contains("app/dist"),
        "the release cache must use the app UI output directory"
    );
}

#[test]
fn ci_repairs_missing_wasm_pack_binary_despite_cached_cargo_metadata() {
    let workflow = workflow("ci.yml");
    let install_step = named_step(&workflow, "Install WASM test tools");

    assert!(
        install_step.contains("cargo binstall --no-confirm --force --locked wasm-pack"),
        "the WASM tool bootstrap must bypass stale cargo-binstall metadata when wasm-pack is missing"
    );
}
