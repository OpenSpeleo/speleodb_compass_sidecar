//! Initial imports are previewed before any project files are published.
//! Filesystem work belongs to the backend; this component owns only interaction
//! state and applies the same dependency closure as the backend.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use common::compass_import::{
    ImportPreview, ImportSection, ImportSelection, InitialImportOutcome, resolve_selection,
};
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlDialogElement, HtmlElement, HtmlInputElement};
use yew::prelude::*;

use crate::{Error, speleo_db_controller::SPELEO_DB_CONTROLLER};

#[derive(Clone, Debug, PartialEq)]
enum ImportPhase {
    Picking,
    Analyzing,
    Reviewing,
    Importing,
    Failed,
}

#[derive(Clone, PartialEq)]
struct ImportFlow {
    phase: ImportPhase,
    source_path: Option<String>,
    preview: Option<ImportPreview>,
    error: Option<String>,
    needs_refresh: bool,
}

impl Default for ImportFlow {
    fn default() -> Self {
        Self {
            phase: ImportPhase::Picking,
            source_path: None,
            preview: None,
            error: None,
            needs_refresh: false,
        }
    }
}

/// Synchronous guards close the gap between a click and Yew's next render.
/// Generation checks also discard late file-picker and analysis responses.
struct ImportRequests {
    generation: Cell<u64>,
    mounted: Cell<bool>,
    reading: Cell<bool>,
    submitting: Cell<bool>,
    preview_id: Cell<Option<Uuid>>,
}

impl Default for ImportRequests {
    fn default() -> Self {
        Self {
            generation: Cell::new(0),
            mounted: Cell::new(true),
            reading: Cell::new(false),
            submitting: Cell::new(false),
            preview_id: Cell::new(None),
        }
    }
}

impl ImportRequests {
    fn begin(&self) -> u64 {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        generation
    }

    fn accepts(&self, generation: u64) -> bool {
        self.mounted.get() && self.generation.get() == generation
    }

    fn start_import(&self) -> Option<u64> {
        if !self.mounted.get() || self.reading.get() || self.submitting.replace(true) {
            return None;
        }
        Some(self.begin())
    }

    fn start_read(&self) -> Option<u64> {
        if !self.mounted.get() || self.submitting.get() || self.reading.replace(true) {
            return None;
        }
        Some(self.begin())
    }
}

fn release_preview(preview_id: Option<Uuid>) {
    if let Some(preview_id) = preview_id {
        spawn_local(async move {
            if let Err(error) = SPELEO_DB_CONTROLLER.cancel_compass_import(preview_id).await {
                log::warn!("Could not release import preview: {error}");
            }
        });
    }
}

async fn analyze_source(
    project_id: Uuid,
    source_path: String,
    flow: UseStateHandle<ImportFlow>,
    requests: Rc<ImportRequests>,
    generation: u64,
) {
    release_preview(requests.preview_id.take());
    flow.set(ImportFlow {
        phase: ImportPhase::Analyzing,
        source_path: Some(source_path.clone()),
        ..ImportFlow::default()
    });
    let result = SPELEO_DB_CONTROLLER
        .preview_compass_import(project_id, &source_path)
        .await;
    if !requests.accepts(generation) {
        if let Ok(preview) = result {
            release_preview(Some(preview.preview_id));
        }
        return;
    }
    requests.reading.set(false);
    match result {
        Ok(preview) => {
            requests.preview_id.set(Some(preview.preview_id));
            flow.set(ImportFlow {
                phase: ImportPhase::Reviewing,
                source_path: Some(source_path),
                preview: Some(preview),
                error: None,
                needs_refresh: false,
            });
        }
        Err(error) => flow.set(ImportFlow {
            phase: ImportPhase::Failed,
            source_path: Some(source_path),
            error: Some(error.to_string()),
            ..ImportFlow::default()
        }),
    }
}

fn choose_source(
    project_id: Uuid,
    flow: UseStateHandle<ImportFlow>,
    requests: Rc<ImportRequests>,
    on_close: Callback<()>,
) {
    let previous = (*flow).clone();
    let Some(generation) = requests.start_read() else {
        return;
    };
    flow.set(ImportFlow {
        phase: ImportPhase::Picking,
        ..previous.clone()
    });
    spawn_local(async move {
        let result = SPELEO_DB_CONTROLLER.pick_compass_project_file().await;
        if !requests.accepts(generation) {
            return;
        }
        match result {
            Ok(Some(source_path)) => {
                analyze_source(project_id, source_path, flow, requests, generation).await;
            }
            Ok(None) => {
                requests.reading.set(false);
                if previous.source_path.is_some() {
                    flow.set(previous);
                } else {
                    on_close.emit(());
                }
            }
            Err(error) => {
                requests.reading.set(false);
                flow.set(ImportFlow {
                    phase: if previous.preview.is_some() {
                        ImportPhase::Reviewing
                    } else {
                        ImportPhase::Failed
                    },
                    error: Some(error.to_string()),
                    ..previous
                });
            }
        }
    });
}

#[derive(Properties, PartialEq)]
pub struct InitialImportModalProps {
    pub project_id: Uuid,
    pub on_close: Callback<()>,
    pub on_complete: Callback<(InitialImportOutcome, usize)>,
}

#[function_component(InitialImportModal)]
pub fn initial_import_modal(props: &InitialImportModalProps) -> Html {
    let flow = use_state(ImportFlow::default);
    let requests = use_state(|| Rc::new(ImportRequests::default()));
    {
        let project_id = props.project_id;
        let flow = flow.clone();
        let requests = (*requests).clone();
        let on_close = props.on_close.clone();
        use_effect_with((), move |_| {
            choose_source(project_id, flow, requests.clone(), on_close);
            move || {
                requests.mounted.set(false);
                requests.begin();
                if !requests.submitting.get() {
                    release_preview(requests.preview_id.take());
                }
            }
        });
    }

    let on_cancel = {
        let requests = (*requests).clone();
        let on_close = props.on_close.clone();
        Callback::from(move |()| {
            if requests.submitting.get() {
                return;
            }
            requests.begin();
            release_preview(requests.preview_id.take());
            on_close.emit(());
        })
    };
    let on_choose = {
        let project_id = props.project_id;
        let flow = flow.clone();
        let requests = (*requests).clone();
        let on_close = props.on_close.clone();
        Callback::from(move |()| {
            if !requests.submitting.get() {
                choose_source(project_id, flow.clone(), requests.clone(), on_close.clone());
            }
        })
    };
    let on_refresh = {
        let project_id = props.project_id;
        let flow = flow.clone();
        let requests = (*requests).clone();
        Callback::from(move |()| {
            let Some(source_path) = flow.source_path.clone() else {
                return;
            };
            if requests.submitting.get() {
                return;
            }
            let Some(generation) = requests.start_read() else {
                return;
            };
            let flow = flow.clone();
            let requests = requests.clone();
            spawn_local(async move {
                analyze_source(project_id, source_path, flow, requests, generation).await;
            });
        })
    };
    let on_confirm = {
        let flow = flow.clone();
        let requests = (*requests).clone();
        let on_complete = props.on_complete.clone();
        Callback::from(move |explicit: Vec<usize>| {
            if requests.submitting.get()
                || flow.phase != ImportPhase::Reviewing
                || flow.needs_refresh
                || explicit.is_empty()
            {
                return;
            }
            let Some(preview) = &flow.preview else {
                return;
            };
            if requests.preview_id.get() != Some(preview.preview_id) {
                return;
            }
            let Ok(selection) =
                resolve_selection(&preview.sections, &explicit.iter().copied().collect())
            else {
                return;
            };
            let count = selection.included.len();
            let preview_id = preview.preview_id;
            let previous = (*flow).clone();
            let Some(generation) = requests.start_import() else {
                return;
            };
            flow.set(ImportFlow {
                phase: ImportPhase::Importing,
                error: None,
                ..previous.clone()
            });
            let flow = flow.clone();
            let requests = requests.clone();
            let on_complete = on_complete.clone();
            spawn_local(async move {
                let result = SPELEO_DB_CONTROLLER
                    .confirm_compass_import(preview_id, explicit)
                    .await;
                requests.submitting.set(false);
                if !requests.accepts(generation) {
                    release_preview(requests.preview_id.take());
                    return;
                }
                match result {
                    Ok(outcome) => {
                        requests.preview_id.set(None);
                        on_complete.emit((outcome, count));
                    }
                    Err(error) => flow.set(ImportFlow {
                        phase: ImportPhase::Reviewing,
                        needs_refresh: matches!(error, Error::ImportSourceChanged(_)),
                        error: Some(error.to_string()),
                        ..previous
                    }),
                }
            });
        })
    };

    // The native file chooser appears first. Once analysis starts, keep the
    // same modal surface through review and upload rather than stacking dialogs.
    if flow.phase == ImportPhase::Picking && flow.source_path.is_none() && flow.error.is_none() {
        return html! {};
    }
    let importing = flow.phase == ImportPhase::Importing;
    let working = matches!(flow.phase, ImportPhase::Picking | ImportPhase::Analyzing);
    html! {
        <ImportDialog on_cancel={on_cancel.clone()} busy={importing}>
            {
                if let Some(preview) = &flow.preview {
                    html! {
                        <SectionSelector
                            key={preview.preview_id.to_string()}
                            preview={preview.clone()}
                            on_confirm={on_confirm}
                            on_cancel={on_cancel}
                            on_choose_file={on_choose}
                            on_refresh={on_refresh}
                            busy={importing || working}
                            needs_refresh={flow.needs_refresh}
                            error={flow.error.clone()}
                        />
                    }
                } else {
                    html! {
                        <>
                            <header class="import-selector__header">
                                <div class="import-selector__eyebrow">{"COMPASS IMPORT"}</div>
                                <h2 id="import-selector-title" tabindex="-1">{"Choose sections to import"}</h2>
                                <p id="import-selector-description">{"Bring the parts of your survey you need into this project."}</p>
                            </header>
                            <div class="import-selector__loading" aria-live="polite" aria-busy={working.to_string()}>
                                if working {
                                    <span class="import-selector__spinner" aria-hidden="true" />
                                    <h3>{"Reading your project"}</h3>
                                    <p>{"Checking sections and their connections…"}</p>
                                } else {
                                    <h3>{"This project needs a closer look"}</h3>
                                    <p role="alert">{flow.error.clone().unwrap_or_default()}</p>
                                }
                            </div>
                            <footer class="import-selector__footer">
                                <span class="import-selector__footnote">{"Your original files stay untouched."}</span>
                                <div class="import-selector__actions">
                                    <button type="button" class="import-selector__secondary" onclick={on_cancel.reform(|_| ())}>{"Cancel"}</button>
                                    if !working {
                                        <button type="button" class="import-selector__secondary" onclick={on_choose.reform(|_| ())}>{"Choose another file"}</button>
                                        if flow.source_path.is_some() {
                                            <button type="button" class="import-selector__primary" onclick={on_refresh.reform(|_| ())}>{"Try again"}</button>
                                        }
                                    }
                                </div>
                            </footer>
                        </>
                    }
                }
            }
        </ImportDialog>
    }
}

#[derive(Properties, PartialEq)]
pub struct ImportDialogProps {
    pub children: Children,
    pub on_cancel: Callback<()>,
    #[prop_or_default]
    pub busy: bool,
}

#[function_component(ImportDialog)]
pub fn import_dialog(props: &ImportDialogProps) -> Html {
    let dialog_ref = use_node_ref();
    {
        let dialog_ref = dialog_ref.clone();
        use_effect_with((), move |_| {
            let dialog = dialog_ref.cast::<HtmlDialogElement>();
            if let Some(dialog) = &dialog {
                if let Err(error) = dialog.show_modal() {
                    log::error!("Could not open import dialog: {error:?}");
                }
                if let Ok(Some(heading)) = dialog.query_selector("#import-selector-title") {
                    use wasm_bindgen::JsCast;
                    if let Some(heading) = heading.dyn_ref::<HtmlElement>() {
                        let _ = heading.focus();
                    }
                }
            }
            move || {
                if let Some(dialog) = dialog {
                    dialog.close();
                }
            }
        });
    }
    let oncancel = {
        let on_cancel = props.on_cancel.clone();
        let busy = props.busy;
        Callback::from(move |event: Event| {
            event.prevent_default();
            if !busy {
                on_cancel.emit(());
            }
        })
    };
    html! {
        <dialog
            ref={dialog_ref}
            class="import-selector"
            aria-labelledby="import-selector-title"
            aria-describedby="import-selector-description"
            aria-modal="true"
            {oncancel}
        >
            {for props.children.iter()}
        </dialog>
    }
}

fn all_section_ids(preview: &ImportPreview) -> BTreeSet<usize> {
    preview.sections.iter().map(|section| section.id).collect()
}

fn section_matches(section: &ImportSection, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || section.name.to_lowercase().contains(&query)
        || section.relative_path.to_lowercase().contains(&query)
}

fn toggle_section(
    preview: &ImportPreview,
    explicit: &BTreeSet<usize>,
    id: usize,
) -> BTreeSet<usize> {
    let mut updated = explicit.clone();
    if preview.full_import_reason.is_some() {
        return updated;
    }
    let Ok(selection) = resolve_selection(&preview.sections, explicit) else {
        return updated;
    };
    if selection.required_by.contains_key(&id)
        || !preview.sections.iter().any(|section| section.id == id)
    {
        return updated;
    }
    if !updated.remove(&id) {
        updated.insert(id);
    }
    updated
}

#[derive(Properties, PartialEq, Clone)]
pub struct SectionSelectorProps {
    pub preview: ImportPreview,
    pub on_confirm: Callback<Vec<usize>>,
    pub on_cancel: Callback<()>,
    pub on_choose_file: Callback<()>,
    pub on_refresh: Callback<()>,
    #[prop_or_default]
    pub busy: bool,
    #[prop_or_default]
    pub needs_refresh: bool,
    #[prop_or_default]
    pub error: Option<String>,
}

#[function_component(SectionSelector)]
pub fn section_selector(props: &SectionSelectorProps) -> Html {
    let explicit = use_state(|| all_section_ids(&props.preview));
    let query = use_state(String::new);
    let expanded = use_state(BTreeSet::<usize>::new);
    let heading_ref = use_node_ref();
    {
        let heading_ref = heading_ref.clone();
        use_effect_with((), move |_| {
            if let Some(heading) = heading_ref.cast::<HtmlElement>() {
                let _ = heading.focus();
            }
            || ()
        });
    }
    let preview = use_memo(props.preview.clone(), Clone::clone);
    let selection_result = use_memo(
        (preview.clone(), (*explicit).clone()),
        |(preview, explicit)| resolve_selection(&preview.sections, explicit),
    );
    let graph_error = selection_result
        .as_ref()
        .as_ref()
        .err()
        .map(ToString::to_string);
    let empty_selection = ImportSelection::default();
    let selection = selection_result
        .as_ref()
        .as_ref()
        .unwrap_or(&empty_selection);
    let by_id: BTreeMap<_, _> = preview
        .sections
        .iter()
        .map(|section| (section.id, section))
        .collect();
    let connection_reasons = use_memo(preview.clone(), |preview| {
        let mut reasons: BTreeMap<usize, Vec<(usize, String)>> = BTreeMap::new();
        for candidate in &preview.sections {
            for dependency in &candidate.dependencies {
                reasons.entry(dependency.section_id).or_default().push((
                    candidate.id,
                    format!("{}: {}", candidate.name, dependency.reason),
                ));
            }
        }
        reasons
    });
    let total = props.preview.sections.len();
    let count = selection.included.len();
    let automatic = count.saturating_sub(explicit.len());
    let full_only = props.preview.full_import_reason.is_some();
    let disabled = props.busy || props.needs_refresh;
    let all_selected = explicit.len() == total;
    let matches: Vec<_> = props
        .preview
        .sections
        .iter()
        .filter(|section| section_matches(section, &query))
        .collect();

    let on_search = {
        let query = query.clone();
        Callback::from(move |event: InputEvent| {
            query.set(event.target_unchecked_into::<HtmlInputElement>().value());
        })
    };
    let on_select_all = {
        let explicit = explicit.clone();
        let ids = all_section_ids(&props.preview);
        Callback::from(move |_| explicit.set(ids.clone()))
    };
    let on_clear = {
        let explicit = explicit.clone();
        Callback::from(move |_| explicit.set(BTreeSet::new()))
    };
    let on_confirm = {
        let explicit = explicit.clone();
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_| {
            if !disabled && !explicit.is_empty() && graph_error.is_none() {
                on_confirm.emit(explicit.iter().copied().collect());
            }
        })
    };
    let error = props.error.clone().or_else(|| {
        selection_result
            .as_ref()
            .as_ref()
            .err()
            .map(ToString::to_string)
    });

    html! {
        <>
            <header class="import-selector__header">
                <div class="import-selector__eyebrow">{"COMPASS IMPORT"}</div>
                <h2 ref={heading_ref} id="import-selector-title" tabindex="-1">{"Choose sections to import"}</h2>
                <p id="import-selector-description">{"Choose the sections you need. Required connections are included automatically."}</p>
                <div class="import-selector__source">
                    <span class="import-selector__file-icon" aria-hidden="true">{"↳"}</span>
                    <div>
                        <strong>{&props.preview.source_name}</strong>
                        <span>{&props.preview.source_directory}</span>
                    </div>
                    <button type="button" class="import-selector__text-button" disabled={props.busy} onclick={props.on_choose_file.reform(|_| ())}>{"Change file"}</button>
                </div>
            </header>
            if let Some(reason) = &props.preview.full_import_reason {
                <div class="import-selector__notice" role="status">
                    <strong>{"This project needs to stay together"}</strong>
                    <p>{reason}</p>
                    <p>{"You can import the complete project. All sections are included."}</p>
                </div>
            }
            if let Some(error) = error {
                <div class="import-selector__error" role="alert">
                    <strong>{if props.needs_refresh { "The source files changed" } else { "Import could not finish" }}</strong>
                    <p>{error}</p>
                    if props.needs_refresh {
                        <p>{"Refresh to review the updated project. All sections will be selected again."}</p>
                    }
                    <button type="button" class="import-selector__text-button" disabled={props.busy} onclick={props.on_refresh.reform(|_| ())}>{"Refresh preview"}</button>
                </div>
            }
            <div class="import-selector__toolbar">
                <label class="import-selector__search">
                    <span class="import-selector__sr-only">{"Search sections"}</span>
                    <svg aria-hidden="true" width="17" height="17" viewBox="0 0 20 20" fill="none"><circle cx="8.5" cy="8.5" r="5.5" stroke="currentColor" stroke-width="1.7"/><path d="m13 13 4 4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
                    <input type="search" placeholder="Find a section…" value={(*query).clone()} oninput={on_search} disabled={props.busy}/>
                </label>
                if !full_only {
                    <div class="import-selector__bulk">
                        <button type="button" class="import-selector__text-button" disabled={disabled || all_selected} onclick={on_select_all}>{"Select all"}</button>
                        <span aria-hidden="true">{"·"}</span>
                        <button type="button" class="import-selector__text-button" disabled={disabled || explicit.is_empty()} onclick={on_clear}>{"Clear selection"}</button>
                    </div>
                }
            </div>
            <div class="import-selector__list" aria-label="Project sections" aria-busy={props.busy.to_string()}>
                if matches.is_empty() {
                    <div class="import-selector__no-results">
                        <strong>{"No matching sections"}</strong>
                        <p>{"Try a different name or clear your search."}</p>
                        <button type="button" class="import-selector__text-button" onclick={{let query = query.clone(); Callback::from(move |_| query.set(String::new()))}}>{"Clear search"}</button>
                    </div>
                }
                {for matches.into_iter().map(|section| {
                    let checked = selection.included.contains(&section.id);
                    let required = selection.required_by.get(&section.id);
                    let locked = required.is_some();
                    let names = required.map(|ids| ids.iter().take(2).filter_map(|id| by_id.get(id).map(|section| section.name.as_str())).collect::<Vec<_>>()).unwrap_or_default();
                    let required_count = required.map_or(0, BTreeSet::len);
                    let explanation_id = format!("import-section-reason-{}", section.id);
                    let checkbox_id = format!("import-section-{}", section.id);
                    let required_text = if required_count > 2 {
                        format!("Required by {} and {} others", names[0], required_count - 1)
                    } else {
                        format!("Required by {}", names.join(" and "))
                    };
                    let onchange = {
                        let explicit = explicit.clone();
                        let preview = preview.clone();
                        let section_id = section.id;
                        Callback::from(move |_| {
                            if !disabled {
                                explicit.set(toggle_section(&preview, &explicit, section_id));
                            }
                        })
                    };
                    let ontoggle = {
                        let expanded = expanded.clone();
                        let section_id = section.id;
                        Callback::from(move |event: Event| {
                            let open = event.target_unchecked_into::<web_sys::HtmlDetailsElement>().open();
                            if expanded.contains(&section_id) != open {
                                let mut next = (*expanded).clone();
                                if open { next.insert(section_id); } else { next.remove(&section_id); }
                                expanded.set(next);
                            }
                        })
                    };
                    html! {
                        <div key={section.id} class={classes!("import-selector__row", checked.then_some("import-selector__row--selected"))}>
                            <input
                                id={checkbox_id.clone()}
                                type="checkbox"
                                checked={checked}
                                disabled={disabled || locked || full_only}
                                aria-describedby={locked.then_some(explanation_id.clone())}
                                {onchange}
                            />
                            <div class="import-selector__row-content">
                                <label for={checkbox_id}>
                                    <strong>{&section.name}</strong>
                                    <span class="import-selector__path">{&section.relative_path}</span>
                                </label>
                                if locked {
                                    <details class="import-selector__dependency" open={expanded.contains(&section.id)} {ontoggle}>
                                        <summary id={explanation_id}>{required_text}</summary>
                                        if expanded.contains(&section.id) {
                                        <ul>
                                            {for required.into_iter().flatten().filter_map(|id| by_id.get(id)).map(|root| html!{ <li>{format!("Required by {}", root.name)}</li> })}
                                            {for connection_reasons.get(&section.id).into_iter().flatten().filter(|(id, _)| selection.included.contains(id)).map(|(_, reason)| html!{ <li>{reason}</li> })}
                                        </ul>
                                        }
                                    </details>
                                }
                            </div>
                            if locked {
                                <span class="import-selector__badge">{"Required"}</span>
                            }
                        </div>
                    }
                })}
            </div>
            <div class="import-selector__summary" role="status" aria-live="polite" aria-atomic="true">
                <strong>{format!("{count} of {total} sections")}</strong>
                if automatic > 0 {
                    <span>{format!("{automatic} included automatically")}</span>
                } else if count == 0 {
                    <span>{"Select at least one section to continue."}</span>
                } else if all_selected && !full_only {
                    <span>{"Import everything, or clear the selection to choose a smaller set."}</span>
                } else {
                    <span>{"Your original files stay untouched."}</span>
                }
            </div>
            <footer class="import-selector__footer">
                if props.busy {
                    <div class="import-selector__progress" role="status">
                        <span class="import-selector__spinner import-selector__spinner--small" aria-hidden="true"/>
                        <span>{"Preparing your selection and syncing with SpeleoDB…"}</span>
                    </div>
                } else {
                    <span class="import-selector__footnote">{"A new copy. Just your selection."}</span>
                }
                <div class="import-selector__actions">
                    <button type="button" class="import-selector__secondary" disabled={props.busy} onclick={props.on_cancel.reform(|_| ())}>{"Cancel"}</button>
                    <button type="button" class="import-selector__primary" disabled={disabled || count == 0 || selection.included.is_empty()} onclick={on_confirm}>
                        {if props.busy { "Importing…".to_owned() } else if full_only { "Import complete project".to_owned() } else {format!("Import {count} {}", if count == 1 { "section" } else { "sections" })}}
                    </button>
                </div>
            </footer>
        </>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::compass_import::ImportDependency;

    fn preview() -> ImportPreview {
        ImportPreview {
            preview_id: Uuid::nil(),
            source_name: "Cave.mak".into(),
            source_directory: "Surveys/Cave".into(),
            sections: (0..3)
                .map(|id| ImportSection {
                    id,
                    name: format!("{}.DAT", char::from(b'A' + id as u8)),
                    relative_path: format!("region/{id}.DAT"),
                    dependencies: if id == 1 {
                        vec![ImportDependency {
                            section_id: 0,
                            reason: "Connects at station A1".into(),
                        }]
                    } else {
                        vec![]
                    },
                })
                .collect(),
            full_import_reason: None,
        }
    }

    #[test]
    fn default_all_selection_preserves_explicit_prerequisite() {
        let preview = preview();
        let explicit = all_section_ids(&preview);
        assert_eq!(toggle_section(&preview, &explicit, 0), explicit);
        let explicit = toggle_section(&preview, &explicit, 1);
        assert_eq!(explicit, BTreeSet::from([0, 2]));
        assert_eq!(toggle_section(&preview, &explicit, 0), BTreeSet::from([2]));
    }

    #[test]
    fn selecting_a_dependent_adds_and_removes_only_automatic_dependencies() {
        let preview = preview();
        let explicit = toggle_section(&preview, &BTreeSet::new(), 1);
        assert_eq!(explicit, BTreeSet::from([1]));
        assert_eq!(
            resolve_selection(&preview.sections, &explicit)
                .unwrap()
                .included,
            BTreeSet::from([0, 1])
        );
        assert!(toggle_section(&preview, &explicit, 1).is_empty());
    }

    #[test]
    fn filtering_matches_names_and_paths_without_changing_selection() {
        let preview = preview();
        assert!(section_matches(&preview.sections[0], " a.dat "));
        assert!(section_matches(&preview.sections[0], "REGION/0"));
        assert!(!section_matches(&preview.sections[0], "B.DAT"));
        assert_eq!(all_section_ids(&preview).len(), 3);
    }

    #[test]
    fn full_only_previews_cannot_change_selection() {
        let mut preview = preview();
        preview.full_import_reason = Some("Unsupported directive".into());
        let explicit = all_section_ids(&preview);
        assert_eq!(toggle_section(&preview, &explicit, 2), explicit);
    }

    #[test]
    fn cancelled_or_replaced_requests_cannot_publish_late_results() {
        let requests = ImportRequests::default();
        let first = requests.begin();
        assert!(requests.accepts(first));
        let second = requests.begin();
        assert!(!requests.accepts(first));
        assert!(requests.accepts(second));
        requests.mounted.set(false);
        assert!(!requests.accepts(second));
    }

    #[test]
    fn confirmation_guard_blocks_double_submission_before_rerender() {
        let requests = ImportRequests::default();
        assert!(requests.start_import().is_some());
        assert!(requests.start_import().is_none());
    }

    #[test]
    fn file_picker_and_preview_reads_are_serialized_before_rerender() {
        let requests = ImportRequests::default();
        assert!(requests.start_read().is_some());
        assert!(requests.start_read().is_none());
        assert!(requests.start_import().is_none());
        requests.reading.set(false);
        assert!(requests.start_import().is_some());
        assert!(requests.start_read().is_none());
    }

    fn selector_props() -> SectionSelectorProps {
        SectionSelectorProps {
            preview: preview(),
            on_confirm: Callback::noop(),
            on_cancel: Callback::noop(),
            on_choose_file: Callback::noop(),
            on_refresh: Callback::noop(),
            busy: false,
            needs_refresh: false,
            error: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn renders_labels_dependency_reason_and_full_import_fallback() {
        let html = futures::executor::block_on(
            yew::ServerRenderer::<SectionSelector>::with_props(selector_props).render(),
        );
        assert!(html.contains("Choose sections to import"));
        assert!(html.contains("Required by B.DAT"));
        assert!(
            !html.contains("Connects at station A1"),
            "connection explanations render only when expanded"
        );
        assert!(html.contains("Import 3 sections"));
        assert!(html.contains("aria-describedby=\"import-section-reason-0\""));
        let html = futures::executor::block_on(
            yew::ServerRenderer::<SectionSelector>::with_props(|| {
                let mut props = selector_props();
                props.preview.full_import_reason =
                    Some("Nested project directives require a complete import.".into());
                props
            })
            .render(),
        );
        assert!(html.contains("Import complete project"));
        assert!(html.contains("This project needs to stay together"));
        assert!(!html.contains("Clear selection"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn renders_twenty_sections_with_a_persistent_footer() {
        let html = futures::executor::block_on(
            yew::ServerRenderer::<SectionSelector>::with_props(|| {
                let mut props = selector_props();
                props.preview.source_name = "Mammoth Cave.mak".into();
                props.preview.source_directory = "Surveys / Kentucky / Mammoth Cave".into();
                props
                    .preview
                    .sections
                    .extend((3..20).map(|id| ImportSection {
                        id,
                        name: format!("Passage {id:02}.DAT"),
                        relative_path: format!("North Entrance / Passage {id:02}.DAT"),
                        dependencies: vec![],
                    }));
                props
            })
            .render(),
        );
        assert_eq!(html.matches("type=\"checkbox\"").count(), 20);
        assert!(html.contains("20 of 20 sections"));
        assert!(html.contains("import-selector__footer"));
        // `--nocapture` also provides the actual component markup for visual QA.
        println!("IMPORT_SELECTOR_PREVIEW_START\n{html}\nIMPORT_SELECTOR_PREVIEW_END");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn hundreds_of_connected_sections_do_not_render_hidden_dependency_trees() {
        let html = futures::executor::block_on(
            yew::ServerRenderer::<SectionSelector>::with_props(|| {
                let mut props = selector_props();
                props.preview.sections = (0..500)
                    .map(|id| ImportSection {
                        id,
                        name: format!("Passage {id}.DAT"),
                        relative_path: format!("Passage {id}.DAT"),
                        dependencies: if id > 0 {
                            vec![ImportDependency {
                                section_id: id - 1,
                                reason: "Shared station".into(),
                            }]
                        } else {
                            vec![]
                        },
                    })
                    .collect();
                props
            })
            .render(),
        );
        assert_eq!(html.matches("type=\"checkbox\"").count(), 500);
        assert!(html.contains("500 of 500 sections"));
        assert_eq!(html.matches("<li>").count(), 0);
    }

    #[cfg(target_arch = "wasm32")]
    mod browser {
        use super::*;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
        use web_sys::Element;

        wasm_bindgen_test_configure!(run_in_browser);

        #[function_component(TestSelector)]
        fn test_selector(props: &SectionSelectorProps) -> Html {
            html! {
                <ImportDialog on_cancel={props.on_cancel.clone()} busy={props.busy}>
                    <SectionSelector ..props.clone()/>
                </ImportDialog>
            }
        }

        async fn render_tick() {
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
                    .unwrap();
            });
            wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
        }

        async fn mount(props: SectionSelectorProps) -> (Element, yew::AppHandle<TestSelector>) {
            let document = web_sys::window().unwrap().document().unwrap();
            let root = document.create_element("div").unwrap();
            document.body().unwrap().append_child(&root).unwrap();
            let handle =
                yew::Renderer::<TestSelector>::with_root_and_props(root.clone(), props).render();
            render_tick().await;
            (root, handle)
        }

        fn checkbox(root: &Element, id: usize) -> HtmlInputElement {
            root.query_selector(&format!("#import-section-{id}"))
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap()
        }

        fn button(root: &Element, text: &str) -> HtmlElement {
            let buttons = root.query_selector_all("button").unwrap();
            (0..buttons.length())
                .filter_map(|index| buttons.item(index))
                .find(|node| node.text_content().as_deref() == Some(text))
                .unwrap()
                .dyn_into()
                .unwrap()
        }

        #[wasm_bindgen_test]
        async fn checkbox_interactions_preserve_dependencies_and_submit_explicit_roots() {
            let confirmed = Rc::new(std::cell::RefCell::new(Vec::new()));
            let mut props = selector_props();
            let captured = confirmed.clone();
            props.on_confirm = Callback::from(move |ids| captured.borrow_mut().push(ids));
            let (root, handle) = mount(props).await;
            assert!(checkbox(&root, 0).checked());
            assert!(checkbox(&root, 0).disabled());
            assert!(root.text_content().unwrap().contains("Required by B.DAT"));
            let explanation: HtmlElement = root
                .query_selector("summary")
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap();
            explanation.click();
            render_tick().await;
            render_tick().await;
            assert!(
                root.text_content()
                    .unwrap()
                    .contains("Connects at station A1")
            );

            button(&root, "Clear selection").click();
            render_tick().await;
            assert!(!checkbox(&root, 0).checked());
            assert!(button(&root, "Import 0 sections").has_attribute("disabled"));

            checkbox(&root, 1).click();
            render_tick().await;
            assert!(checkbox(&root, 0).checked());
            assert!(checkbox(&root, 0).disabled());
            assert!(
                root.text_content()
                    .unwrap()
                    .contains("1 included automatically")
            );
            button(&root, "Import 2 sections").click();
            assert_eq!(*confirmed.borrow(), vec![vec![1]]);

            checkbox(&root, 1).click();
            render_tick().await;
            assert!(!checkbox(&root, 0).checked());
            handle.destroy();
            root.remove();
        }

        #[wasm_bindgen_test]
        async fn search_and_bulk_actions_keep_hidden_sections_in_the_selection() {
            let (root, handle) = mount(selector_props()).await;
            let input: HtmlInputElement = root
                .query_selector("input[type=search]")
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap();
            input.set_value("C.DAT");
            input.dispatch_event(&Event::new("input").unwrap()).unwrap();
            render_tick().await;
            assert_eq!(
                root.query_selector_all("input[type=checkbox]")
                    .unwrap()
                    .length(),
                1
            );
            button(&root, "Clear selection").click();
            render_tick().await;
            assert!(root.text_content().unwrap().contains("0 of 3 sections"));
            button(&root, "Select all").click();
            render_tick().await;
            assert!(root.text_content().unwrap().contains("3 of 3 sections"));
            handle.destroy();
            root.remove();
        }

        #[wasm_bindgen_test]
        async fn native_dialog_focus_and_cancel_respect_busy_state() {
            let cancelled = Rc::new(Cell::new(0));
            let mut props = selector_props();
            let captured = cancelled.clone();
            props.on_cancel = Callback::from(move |()| captured.set(captured.get() + 1));
            let (root, mut handle) = mount(props.clone()).await;
            let document = web_sys::window().unwrap().document().unwrap();
            assert_eq!(
                document.active_element().unwrap().id(),
                "import-selector-title"
            );
            let dialog: HtmlDialogElement = root
                .query_selector("dialog")
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap();
            assert!(dialog.open());
            dialog
                .dispatch_event(&Event::new("cancel").unwrap())
                .unwrap();
            assert_eq!(cancelled.get(), 1);

            props.busy = true;
            handle.update(props);
            render_tick().await;
            dialog
                .dispatch_event(&Event::new("cancel").unwrap())
                .unwrap();
            assert_eq!(cancelled.get(), 1);
            assert!(button(&root, "Importing…").has_attribute("disabled"));
            assert!(root.query_selector(".modal").unwrap().is_none());
            handle.destroy();
            root.remove();
        }

        #[wasm_bindgen_test]
        async fn long_warnings_and_errors_keep_import_actions_reachable() {
            let document = web_sys::window().unwrap().document().unwrap();
            let styles = document.create_element("style").unwrap();
            styles.set_text_content(Some(include_str!("../../styles.css")));
            document.body().unwrap().append_child(&styles).unwrap();
            let mut props = selector_props();
            props.preview.full_import_reason = Some(format!(
                "Connections in {}Survey.DAT could not be verified. Import all sections.",
                "directory/".repeat(30)
            ));
            props.error = Some("The server could not complete this request. ".repeat(50));
            let (root, handle) = mount(props).await;
            let dialog: HtmlElement = root
                .query_selector("dialog")
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap();
            let footer: HtmlElement = root
                .query_selector("footer")
                .unwrap()
                .unwrap()
                .dyn_into()
                .unwrap();
            assert!(dialog.scroll_height() > dialog.client_height());
            assert!(
                footer.offset_top() + footer.offset_height()
                    <= dialog.scroll_top() + dialog.client_height() + 1,
                "confirmation and cancellation must stay inside the visible dialog"
            );
            dialog.set_scroll_top(dialog.scroll_height());
            render_tick().await;
            assert!(
                footer.offset_top() + footer.offset_height()
                    <= dialog.scroll_top() + dialog.client_height() + 1,
                "actions must remain reachable after scrolling through the error"
            );
            handle.destroy();
            root.remove();
            styles.remove();
        }

        #[wasm_bindgen_test]
        async fn source_changes_require_refresh_and_full_import_disables_filtering() {
            let mut props = selector_props();
            props.needs_refresh = true;
            props.error = Some("A.DAT changed since this preview was prepared.".into());
            let (root, mut handle) = mount(props.clone()).await;
            assert!(button(&root, "Import 3 sections").has_attribute("disabled"));
            assert!(
                root.text_content()
                    .unwrap()
                    .contains("The source files changed")
            );
            assert!(root.query_selector("[role=alert]").unwrap().is_some());
            props.needs_refresh = false;
            props.error = None;
            props.preview.full_import_reason = Some("Nested project files".into());
            handle.update(props);
            render_tick().await;
            assert!(!root.text_content().unwrap().contains("Clear selection"));
            assert!(checkbox(&root, 2).disabled());
            assert!(!button(&root, "Import complete project").has_attribute("disabled"));
            handle.destroy();
            root.remove();
        }
    }
}
