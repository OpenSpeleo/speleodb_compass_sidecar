use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::speleo_db_controller::{SPELEO_DB_CONTROLLER, Project};

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
	#[prop_or_default]
	pub on_select: Callback<String>,
}

#[function_component(ProjectListing)]
pub fn project_listing(props: &ProjectListingProps) -> Html {
	let projects: UseStateHandle<Vec<Project>> = use_state(|| vec![]);
	let loading = use_state(|| true);
	let error = use_state(|| None::<String>);
	
	// Fetch projects on mount
	{
		let projects = projects.clone();
		let loading = loading.clone();
		let error = error.clone();
		use_effect_with((), move |_| {
			spawn_local(async move {
				match SPELEO_DB_CONTROLLER.fetch_projects().await {
					Ok(project_list) => {
						projects.set(project_list);
						loading.set(false);
					}
					Err(e) => {
						error.set(Some(e));
						loading.set(false);
					}
				}
			});
			|| ()
		});
	}
	
	// Button handlers
	let on_create_new = Callback::from(move |_| {
		// Placeholder - will be implemented later
	});
	
	let on_refresh = {
		let projects = projects.clone();
		let loading = loading.clone();
		let error = error.clone();
		Callback::from(move |_| {
			let projects = projects.clone();
			let loading = loading.clone();
			let error = error.clone();
			loading.set(true);
			error.set(None);
			spawn_local(async move {
				match SPELEO_DB_CONTROLLER.fetch_projects().await {
					Ok(project_list) => {
						projects.set(project_list);
						loading.set(false);
					}
					Err(e) => {
						error.set(Some(e));
						loading.set(false);
					}
				}
			});
		})
	};
	
	// Render the UI
	if *loading {
		html! {
			<section style="width:100%;">
				<h2>{"Project Listing"}</h2>
				<div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
					<button disabled={true}>{"Create New Project"}</button>
					<button disabled={true}>{"Refresh Projects"}</button>
				</div>
				<p>{"Loading projects..."}</p>
			</section>
		}
	} else if let Some(err_msg) = &*error {
		html! {
			<section style="width:100%;">
				<h2>{"Project Listing"}</h2>
				<div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
					<button onclick={on_create_new.clone()}>{"Create New Project"}</button>
					<button onclick={on_refresh.clone()}>{"Refresh Projects"}</button>
				</div>
				<div class="error-message" style="color: red; padding: 12px; border: 1px solid red; border-radius: 4px;">
					<strong>{"Error: "}</strong>
					<span>{ err_msg }</span>
				</div>
			</section>
		}
	} else {
		html! {
			<section style="width:100%;">
				<h2>{"Project Listing"}</h2>
				<div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
					<button onclick={on_create_new.clone()}>{"Create New Project"}</button>
					<button onclick={on_refresh.clone()}>{"Refresh Projects"}</button>
				</div>
				<div class="projects-list" style="display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
					{ for projects.iter().map(|project| {
						let project_id = project.id.clone();
						let on_select = props.on_select.clone();
						let on_card_click = Callback::from(move |_| {
							on_select.emit(project_id.clone());
						});
						
						let is_locked = project.active_mutex.is_some();
						let lock_status = if is_locked { "🔒 Locked" } else { "🔓 Unlocked" };
						let lock_color = if is_locked { "#ff6b6b" } else { "#51cf66" };
						
						html! {
							<div 
								class="project-card" 
								onclick={on_card_click}
								style={format!(
									"border: 1px solid #ddd; \
									 border-radius: 8px; \
									 padding: 16px; \
									 cursor: pointer; \
									 transition: all 0.2s; \
									 background-color: white; \
									 box-shadow: 0 2px 4px rgba(0,0,0,0.1); \
									 display: flex; \
									 justify-content: space-between; \
									 align-items: center;"
								)}
							>
								<h3 style="margin: 0; font-size: 18px; color: #2c3e50;">
									{ &project.name }
								</h3>
								<div style="display: flex; gap: 12px; align-items: center;">
									<span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;",
                                    if project.permission == "ADMIN" { " #ff7f00" } else if project.permission == "READ_AND_WRITE" { "#228be6" } else { "#868e96" }
									)}>
										{ &project.permission }
									</span>
									<span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;", lock_color)}>
										{ lock_status }
									</span>
								</div>
							</div>
						}
					})}
				</div>
			</section>
		}
	}
}


