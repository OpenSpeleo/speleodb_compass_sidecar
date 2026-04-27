use crate::components::create_project_modal::CreateProjectModal;
use crate::components::project_listing_item::ProjectListingItem;
use common::ui_state::{ProjectStatus, UiState};
use std::cmp::Ordering;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
    pub ui_state: UiState,
}

/// Available sort modes for the project list. `Name` is the default
/// because it gives a stable, alphabetical ordering that does not
/// shuffle as the backend refreshes `modified_date` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortMode {
    Name,
    Modified,
}

/// Case-insensitive ascending name comparator. Pulled out so it can be
/// unit-tested without having to construct full `ProjectStatus` values.
fn cmp_project_name(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Descending comparator over ISO-8601 `modified_date` strings. Lexical
/// order matches chronological order for fixed-width ISO-8601 timestamps,
/// which is the format the backend emits (see `state.rs`).
fn cmp_modified_date_desc(a: &str, b: &str) -> Ordering {
    b.cmp(a)
}

fn sort_projects(mode: SortMode, projects: &mut [ProjectStatus]) {
    match mode {
        SortMode::Name => projects.sort_by(|a, b| cmp_project_name(a.name(), b.name())),
        SortMode::Modified => {
            projects.sort_by(|a, b| cmp_modified_date_desc(a.modified_date(), b.modified_date()))
        }
    }
}

fn sort_button_style(active: bool) -> &'static str {
    if active {
        "background-color: #2563eb; color: #f6f6f6; border: 1px solid #2563eb; \
         padding: 6px 14px; border-radius: 6px; font-size: 13px; font-weight: 500; \
         cursor: pointer; box-shadow: none;"
    } else {
        "background-color: transparent; color: #cbd5e1; border: 1px solid #475569; \
         padding: 6px 14px; border-radius: 6px; font-size: 13px; font-weight: 500; \
         cursor: pointer; box-shadow: none;"
    }
}

#[function_component(ProjectListing)]
pub fn project_listing(ProjectListingProps { ui_state }: &ProjectListingProps) -> Html {
    let error = use_state(|| None::<String>);
    let show_create_modal = use_state(|| false);
    let sort_mode = use_state(|| SortMode::Name);
    let user_email = ui_state.user_email.clone().unwrap();
    // Button handlers
    let on_create_new = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(true);
        })
    };

    // Modal handlers
    let on_close_modal = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_: ()| {
            show_create_modal.set(false);
        })
    };

    let on_sort_name = {
        let sort_mode = sort_mode.clone();
        Callback::from(move |_| sort_mode.set(SortMode::Name))
    };
    let on_sort_modified = {
        let sort_mode = sort_mode.clone();
        Callback::from(move |_| sort_mode.set(SortMode::Modified))
    };

    let sorted_projects: Vec<ProjectStatus> = {
        let mut projects: Vec<ProjectStatus> = ui_state.project_status.to_vec();
        sort_projects(*sort_mode, &mut projects);
        projects
    };

    // Render the UI
    if let Some(err_msg) = &*error {
        html! {
            <>
                <section style="width:100%;">
                    <h2>{"Projects"}</h2>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button onclick={on_create_new.clone()} style="background-color: #2563eb; color: #f6f6f6;">{"Create New Project"}</button>
                    </div>
                    <div class="error-message" style="color: red; padding: 12px; border: 1px solid red; border-radius: 4px;">
                        <strong>{"Error: "}</strong>
                        <span>{ err_msg }</span>
                    </div>
                </section>
                {
                    if *show_create_modal {
                        html! { <CreateProjectModal on_close={on_close_modal}  /> }
                    } else {
                        html! {}
                    }
                }
            </>
        }
    } else {
        html! {
            <>
            <section style="width: 100%;">
                <div style="display:flex; justify-content:space-between;align-items:center;">
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <h2 class={classes!("vertically-centered-text")} >{"Projects"}</h2>
                    </div>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button onclick={on_create_new.clone()} style="background-color: #2563eb; color: #f6f6f6;">{"Create New Project"}</button>
                    </div>
                </div>
                <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 12px;">
                    <span style="color: #94a3b8; font-size: 13px;">{"Sort by:"}</span>
                    <button
                        onclick={on_sort_name}
                        style={sort_button_style(*sort_mode == SortMode::Name)}
                    >
                        {"Name"}
                    </button>
                    <button
                        onclick={on_sort_modified}
                        style={sort_button_style(*sort_mode == SortMode::Modified)}
                    >
                        {"Most Recent"}
                    </button>
                </div>
                <div class="projects-list" style=" display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
                    { for sorted_projects.iter().map(|project| {
                        return html! {
                            <ProjectListingItem project={project.clone()} user_email={user_email.clone()} />
                        };
                    })}
                </div>
            </section>

                {
                    if *show_create_modal {
                        html! {
                            <CreateProjectModal
                                on_close={on_close_modal}
                            />
                        }
                    } else {
                        html! {}
                    }
                }
            </>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn project_name_sort_is_case_insensitive_ascending() {
        assert_eq!(cmp_project_name("alpha", "Beta"), Ordering::Less);
        assert_eq!(cmp_project_name("BETA", "alpha"), Ordering::Greater);
        assert_eq!(cmp_project_name("Alpha", "alpha"), Ordering::Equal);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn modified_date_sort_is_descending() {
        let newer = "2026-04-27T10:00:00Z";
        let older = "2026-04-20T10:00:00Z";
        assert_eq!(cmp_modified_date_desc(newer, older), Ordering::Less);
        assert_eq!(cmp_modified_date_desc(older, newer), Ordering::Greater);
        assert_eq!(cmp_modified_date_desc(newer, newer), Ordering::Equal);
    }

    fn make_project(name: &str, modified: &str) -> ProjectStatus {
        use common::api_types::{ProjectInfo, ProjectType};
        use common::ui_state::LocalProjectStatus;
        use uuid::Uuid;
        ProjectStatus::new(
            LocalProjectStatus::Unknown,
            ProjectInfo {
                id: Uuid::new_v4(),
                name: name.to_string(),
                description: String::new(),
                is_active: true,
                permission: "READ_AND_WRITE".to_string(),
                active_mutex: None,
                country: "US".to_string(),
                created_by: "tester".to_string(),
                creation_date: modified.to_string(),
                modified_date: modified.to_string(),
                latitude: None,
                longitude: None,
                fork_from: None,
                visibility: "PUBLIC".to_string(),
                exclude_geojson: false,
                latest_commit: None,
                project_type: ProjectType::Compass,
            },
        )
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn sort_projects_by_name_orders_case_insensitively() {
        let mut projects = vec![
            make_project("Charlie", "2026-04-01T00:00:00Z"),
            make_project("alpha", "2026-04-03T00:00:00Z"),
            make_project("Bravo", "2026-04-02T00:00:00Z"),
        ];
        sort_projects(SortMode::Name, &mut projects);
        let order: Vec<&str> = projects.iter().map(|p| p.name()).collect();
        assert_eq!(order, vec!["alpha", "Bravo", "Charlie"]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn sort_projects_by_modified_puts_most_recent_first() {
        let mut projects = vec![
            make_project("Old", "2026-04-01T00:00:00Z"),
            make_project("Newest", "2026-04-27T00:00:00Z"),
            make_project("Middle", "2026-04-15T00:00:00Z"),
        ];
        sort_projects(SortMode::Modified, &mut projects);
        let order: Vec<&str> = projects.iter().map(|p| p.name()).collect();
        assert_eq!(order, vec!["Newest", "Middle", "Old"]);
    }

    /// Locks in the use of `sort_by` (stable) over `sort_unstable_by`.
    /// Two rows that compare equal under the active comparator must
    /// retain their incoming order; this gives the listing a
    /// deterministic secondary sort without paying for a multi-key
    /// comparator.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn sort_projects_by_name_is_stable_for_equal_keys() {
        let mut projects = vec![
            make_project("alpha", "2026-04-01T00:00:00Z"),
            make_project("Alpha", "2026-04-15T00:00:00Z"),
            make_project("ALPHA", "2026-04-27T00:00:00Z"),
        ];
        sort_projects(SortMode::Name, &mut projects);
        let order: Vec<&str> = projects.iter().map(|p| p.modified_date()).collect();
        assert_eq!(
            order,
            vec![
                "2026-04-01T00:00:00Z",
                "2026-04-15T00:00:00Z",
                "2026-04-27T00:00:00Z",
            ],
            "case-equal names should retain their incoming order"
        );
    }
}
