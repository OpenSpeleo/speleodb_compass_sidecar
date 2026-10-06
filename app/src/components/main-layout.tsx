import type { UiState } from "../lib/types";
import { ProjectDetails } from "./project-details";
import { ProjectListing } from "./project-listing";

export function MainLayout({ uiState }: { uiState: UiState }) {
  const hasSelection =
    uiState.user_email !== null && uiState.selected_project_id !== null;
  const selected = hasSelection
    ? uiState.project_status.find(
        (project) => project.info.id === uiState.selected_project_id,
      )
    : undefined;
  if (hasSelection && selected === undefined)
    throw new Error("Selected project not found in UI state.");
  return (
    <main className="container">
      <header
        style={{
          display: "flex",
          justifyContent: "space-around",
          alignItems: "center",
          flexDirection: "row",
          marginBottom: "24px",
          width: "100%",
        }}
      >
        <div>
          <h1 className="vertically-centered-text">SpeleoDB Compass Sidecar</h1>
        </div>
      </header>
      <section style={{ width: "100%" }}>
        {selected && uiState.user_email !== null ? (
          <ProjectDetails
            key={selected.info.id}
            project={selected}
            userEmail={uiState.user_email}
            compassOpen={uiState.compass_open}
            projectDownloading={uiState.project_downloading}
          />
        ) : (
          <ProjectListing uiState={uiState} />
        )}
      </section>
    </main>
  );
}
