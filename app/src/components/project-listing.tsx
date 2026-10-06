import { useMemo, useState, type CSSProperties } from "react";
import type { ProjectStatus, UiState } from "../lib/types";
import { CreateProjectModal } from "./create-project-modal";
import { ProjectListingItem } from "./project-listing-item";
import { compareUnicode } from "./project-status";

/** Name is the stable default; remote modified-date refreshes must not shuffle the list. */
export type SortMode = "Name" | "Modified";
/** Case-insensitive ascending comparison, independently testable from project records. */
export const compareProjectName = (a: string, b: string) =>
  compareUnicode(a.toLowerCase(), b.toLowerCase());
/** Backend fixed-width ISO timestamps sort chronologically by lexical order. */
export const compareModifiedDateDesc = (a: string, b: string) =>
  compareUnicode(b, a);
export function sortProjects(
  mode: SortMode,
  projects: readonly ProjectStatus[],
): ProjectStatus[] {
  // Stable sorting preserves incoming order for equal keys without another comparator.
  return [...projects].sort((a, b) =>
    mode === "Name"
      ? compareProjectName(a.info.name, b.info.name)
      : compareModifiedDateDesc(a.info.modified_date, b.info.modified_date),
  );
}
function sortButtonStyle(active: boolean): CSSProperties {
  return {
    backgroundColor: active ? "#2563eb" : "transparent",
    color: active ? "#f6f6f6" : "#cbd5e1",
    border: `1px solid ${active ? "#2563eb" : "#475569"}`,
    padding: "6px 14px",
    borderRadius: "6px",
    fontSize: "13px",
    fontWeight: 500,
    cursor: "pointer",
    boxShadow: "none",
  };
}
export function ProjectListing({ uiState }: { uiState: UiState }) {
  // Modal and sort state live with the list and reset when navigating away.
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [sortMode, setSortMode] = useState<SortMode>("Name");
  const projects = useMemo(
    () => sortProjects(sortMode, uiState.project_status),
    [sortMode, uiState.project_status],
  );
  return (
    <>
      <section style={{ width: "100%" }}>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              gap: "12px",
              marginBottom: "16px",
            }}
          >
            <h2 className="vertically-centered-text">Projects</h2>
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              gap: "12px",
              marginBottom: "16px",
            }}
          >
            <button
              onClick={() => setShowCreateModal(true)}
              style={{ backgroundColor: "#2563eb", color: "#f6f6f6" }}
            >
              Create New Project
            </button>
          </div>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            marginBottom: "12px",
          }}
        >
          <span style={{ color: "#94a3b8", fontSize: "13px" }}>Sort by:</span>
          <button
            onClick={() => setSortMode("Name")}
            style={sortButtonStyle(sortMode === "Name")}
          >
            Name
          </button>
          <button
            onClick={() => setSortMode("Modified")}
            style={sortButtonStyle(sortMode === "Modified")}
          >
            Most Recent
          </button>
        </div>
        <div
          className="projects-list"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "12px",
            marginTop: "16px",
          }}
        >
          {projects.map((project) => (
            <ProjectListingItem
              key={project.info.id}
              project={project}
              userEmail={uiState.user_email!}
            />
          ))}
        </div>
      </section>
      {showCreateModal && (
        <CreateProjectModal onClose={() => setShowCreateModal(false)} />
      )}
    </>
  );
}
