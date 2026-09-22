//! Shared contract and deterministic selection rules for initial Compass imports.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, api_types::ProjectSaveResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportDependency {
    pub section_id: usize,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportSection {
    pub id: usize,
    pub name: String,
    pub relative_path: String,
    pub dependencies: Vec<ImportDependency>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportPreview {
    pub preview_id: Uuid,
    pub source_name: String,
    pub source_directory: String,
    pub sections: Vec<ImportSection>,
    /// A complete import is possible, but selecting a subset cannot be verified.
    pub full_import_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InitialImportOutcome {
    Synced { save_result: ProjectSaveResult },
    LocalOnly { error: Error },
    UploadedNeedsRefresh { error: Error },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportSelection {
    pub included: BTreeSet<usize>,
    /// Explicitly selected roots requiring each section (excluding itself).
    pub required_by: BTreeMap<usize, BTreeSet<usize>>,
}

/// Resolve the same graph in the UI and backend. Never trust frontend closure.
/// Classic MAK dependencies must point backwards in original source order.
pub fn resolve_selection(
    sections: &[ImportSection],
    explicit: &BTreeSet<usize>,
) -> Result<ImportSelection, Error> {
    let by_id: BTreeMap<_, _> = sections
        .iter()
        .map(|section| (section.id, section))
        .collect();
    if by_id.len() != sections.len()
        || sections.iter().any(|section| {
            section.dependencies.iter().any(|dependency| {
                dependency.section_id >= section.id || !by_id.contains_key(&dependency.section_id)
            })
        })
    {
        return Err(Error::CompassProject(
            "Invalid import dependency graph".into(),
        ));
    }
    let ordered: Vec<_> = by_id.values().copied().collect();
    let positions: BTreeMap<_, _> = ordered
        .iter()
        .enumerate()
        .map(|(index, section)| (section.id, index))
        .collect();
    let roots: Vec<_> = explicit.iter().copied().collect();
    // Propagate root bitsets backwards through the DAG once. Rewalking the
    // graph for each selected root becomes expensive for dense, large projects.
    let mut required = vec![vec![0_u64; roots.len().div_ceil(64)]; ordered.len()];
    for (root_index, root) in roots.iter().enumerate() {
        let position = positions
            .get(root)
            .ok_or_else(|| Error::CompassProject("Unknown import section".into()))?;
        required[*position][root_index / 64] |= 1 << (root_index % 64);
    }
    for index in (0..ordered.len()).rev() {
        if required[index].iter().all(|word| *word == 0) {
            continue;
        }
        let root_bits = required[index].clone();
        for dependency in &ordered[index].dependencies {
            let target = positions[&dependency.section_id];
            for (target_word, source_word) in required[target].iter_mut().zip(&root_bits) {
                *target_word |= *source_word;
            }
        }
    }
    let mut selection = ImportSelection::default();
    for (index, section) in ordered.iter().enumerate() {
        for (root_index, root) in roots.iter().enumerate() {
            if required[index][root_index / 64] & (1 << (root_index % 64)) != 0 {
                selection.included.insert(section.id);
                if *root != section.id {
                    selection
                        .required_by
                        .entry(section.id)
                        .or_default()
                        .insert(*root);
                }
            }
        }
    }
    Ok(selection)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections() -> Vec<ImportSection> {
        [vec![], vec![0], vec![1], vec![0]]
            .into_iter()
            .enumerate()
            .map(|(id, dependencies)| ImportSection {
                id,
                name: format!("{id}.dat"),
                relative_path: format!("{id}.dat"),
                dependencies: dependencies
                    .into_iter()
                    .map(|section_id| ImportDependency {
                        section_id,
                        reason: "Shared station".into(),
                    })
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn transitive_and_shared_requirements_track_explicit_roots() {
        let result = resolve_selection(&sections(), &BTreeSet::from([2, 3])).unwrap();
        assert_eq!(result.included, BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(result.required_by[&0], BTreeSet::from([2, 3]));
        assert_eq!(result.required_by[&1], BTreeSet::from([2]));
        let result = resolve_selection(&sections(), &BTreeSet::from([3])).unwrap();
        assert_eq!(result.included, BTreeSet::from([0, 3]));
    }

    #[test]
    fn explicit_prerequisites_survive_dependent_removal_and_clear_removes_all() {
        let initial = resolve_selection(&sections(), &BTreeSet::from([0, 1])).unwrap();
        assert_eq!(initial.required_by[&0], BTreeSet::from([1]));
        let independent = resolve_selection(&sections(), &BTreeSet::from([0])).unwrap();
        assert_eq!(independent.included, BTreeSet::from([0]));
        assert!(independent.required_by.is_empty());
        assert_eq!(
            resolve_selection(&sections(), &BTreeSet::new()).unwrap(),
            ImportSelection::default()
        );
    }

    #[test]
    fn rejects_unknown_ids_and_non_classic_graphs() {
        assert!(resolve_selection(&sections(), &BTreeSet::from([100])).is_err());
        let mut invalid = sections();
        invalid[0].dependencies.push(ImportDependency {
            section_id: 2,
            reason: "cycle".into(),
        });
        assert!(resolve_selection(&invalid, &BTreeSet::from([2])).is_err());
    }

    #[test]
    fn preview_contract_round_trips() {
        let preview = ImportPreview {
            preview_id: Uuid::nil(),
            source_name: "Cave.mak".into(),
            source_directory: "Surveys".into(),
            sections: sections(),
            full_import_reason: None,
        };
        assert_eq!(
            serde_json::from_str::<ImportPreview>(&serde_json::to_string(&preview).unwrap())
                .unwrap(),
            preview
        );
    }

    #[test]
    fn large_dense_graph_handles_root_sets_across_word_boundaries() {
        let sections: Vec<_> = (0..500)
            .map(|id| ImportSection {
                id,
                name: format!("{id}.dat"),
                relative_path: format!("{id}.dat"),
                dependencies: (0..id)
                    .map(|section_id| ImportDependency {
                        section_id,
                        reason: "Shared station".into(),
                    })
                    .collect(),
            })
            .collect();
        let all = (0..500).collect();
        let selected = resolve_selection(&sections, &all).unwrap();
        assert_eq!(selected.included, all);
        assert_eq!(selected.required_by[&0].len(), 499);
        assert_eq!(selected.required_by[&498], BTreeSet::from([499]));
        assert!(!selected.required_by.contains_key(&499));
        let last = resolve_selection(&sections, &BTreeSet::from([499])).unwrap();
        assert_eq!(last.included.len(), 500);
        assert_eq!(last.required_by[&0], BTreeSet::from([499]));
    }
}
