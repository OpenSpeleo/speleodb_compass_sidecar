//! Cross-language selection vectors: the backend is the oracle for the UI port.
use std::{collections::BTreeSet, path::Path};

use common::compass_import::{ImportDependency, ImportSection, resolve_selection};
use serde_json::{Value, json};

fn section(id: usize, dependencies: &[usize]) -> ImportSection {
    ImportSection {
        id,
        name: format!("{id}.DAT"),
        relative_path: format!("region/{id}.DAT"),
        dependencies: dependencies
            .iter()
            .map(|id| ImportDependency {
                section_id: *id,
                reason: "Shared station".into(),
            })
            .collect(),
    }
}

fn vector(name: &str, sections: Vec<ImportSection>, roots: &[usize]) -> Value {
    let result = resolve_selection(&sections, &roots.iter().copied().collect::<BTreeSet<_>>());
    let result = match result {
        Ok(selection) => {
            json!({"included": selection.included, "requiredBy": selection.required_by})
        }
        Err(error) => json!({"error": error.to_string()}),
    };
    json!({"name": name, "sections": sections, "explicit": roots, "result": result})
}

#[test]
fn frontend_selection_vectors_match_rust() {
    let graph = || {
        vec![
            section(0, &[]),
            section(1, &[0]),
            section(2, &[1]),
            section(3, &[0]),
        ]
    };
    let vectors = json!([
        vector("transitive and shared", graph(), &[2, 3]),
        vector("all selected", graph(), &[0, 1, 2, 3]),
        vector("explicit prerequisite preserved", graph(), &[0, 1]),
        vector("independent prerequisite", graph(), &[0]),
        vector("clear", graph(), &[]),
        vector("unknown root", graph(), &[100]),
        vector(
            "forward edge",
            vec![section(0, &[1]), section(1, &[])],
            &[1]
        ),
        vector("missing dependency", vec![section(2, &[1])], &[2]),
        vector(
            "duplicate section",
            vec![section(0, &[]), section(0, &[])],
            &[0]
        ),
        vector(
            "unsorted sparse identifiers",
            vec![section(90, &[10]), section(10, &[])],
            &[90]
        ),
        vector("empty graph", vec![], &[]),
        vector(
            "word boundaries",
            (0..130)
                .map(|id| section(id, if id == 0 { &[] } else { &[0] }))
                .collect(),
            &(0..130).collect::<Vec<_>>()
        )
    ]);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../app/src/lib/__fixtures__/import-selection.json");
    if std::env::var_os("UPDATE_IMPORT_SELECTION_FIXTURES").is_some() {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&vectors).unwrap()),
        )
        .unwrap();
    }
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        fixture, vectors,
        "Regenerate intentionally with UPDATE_IMPORT_SELECTION_FIXTURES=1"
    );
}
