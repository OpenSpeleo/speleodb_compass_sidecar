use std::fs;
use std::path::Path;

fn publish_workflow() -> String {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be located directly below the repository root");
    let workflow_path = repository_root.join(".github/workflows/publish.yml");

    fs::read_to_string(&workflow_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", workflow_path.display()))
}

fn named_step<'a>(workflow: &'a str, name: &str) -> &'a str {
    let marker = format!("- name: {name}");
    let start = workflow
        .lines()
        .scan(0, |offset, line| {
            let line_start = *offset;
            *offset += line.len() + 1;
            Some((line_start, line))
        })
        .find_map(|(offset, line)| (line.trim() == marker).then_some((offset, line)))
        .unwrap_or_else(|| panic!("workflow does not contain a `{name}` step"));
    let indentation = start.1.len() - start.1.trim_start().len();
    let remaining = &workflow[start.0..];
    let end = remaining
        .lines()
        .skip(1)
        .scan(start.1.len() + 1, |offset, line| {
            let line_start = *offset;
            *offset += line.len() + 1;
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
fn install_trunk_step_is_authenticated_and_locked() {
    let workflow = publish_workflow();
    let install_step = named_step(&workflow, "Install trunk");

    assert!(
        install_step.contains("GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}"),
        "the Trunk bootstrap must authenticate cargo-binstall GitHub API requests"
    );
    assert!(
        install_step.contains("cargo binstall --no-confirm --locked trunk"),
        "the Trunk source fallback must use its published Cargo.lock"
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
