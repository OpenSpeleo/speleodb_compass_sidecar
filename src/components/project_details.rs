use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectDetailsProps {
	pub project_id: String,
	#[prop_or_default]
	pub on_back: Callback<()>,
}

#[function_component(ProjectDetails)]
pub fn project_details(props: &ProjectDetailsProps) -> Html {
	html! {
		<section style="width:100%;">
			<div style="margin-bottom: 16px;">
				<button onclick={props.on_back.reform(|_| ())}>{"← Back to Projects"}</button>
			</div>
			<h2>{"Project Details"}</h2>
			<p>{format!("Selected Project ID: {}", props.project_id)}</p>
			<p>{"Details view placeholder. We'll flesh this out next."}</p>
		</section>
	}
}


