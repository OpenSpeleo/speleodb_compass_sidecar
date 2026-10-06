import type { ProjectStatus, UiState } from "../lib/types";

export function projectFixture(
  overrides: Partial<ProjectStatus> = {},
): ProjectStatus {
  return {
    local_status: "UpToDate",
    info: {
      id: "00000000-0000-0000-0000-000000000001",
      name: "Cave",
      description: "Survey",
      is_active: true,
      permission: "READ_AND_WRITE",
      active_mutex: null,
      country: "US",
      created_by: "tester",
      creation_date: "2026-04-01T00:00:00Z",
      modified_date: "2026-04-20T00:00:00Z",
      fork_from: null,
      visibility: "PUBLIC",
      exclude_geojson: false,
      latest_commit: null,
      type: "COMPASS",
    },
    ...overrides,
  };
}
export function stateFixture(
  projects: ProjectStatus[] = [projectFixture()],
): UiState {
  return {
    loading_state: "Ready",
    platform: "Windows",
    user_email: "me@example.com",
    project_status: projects,
    selected_project_id: null,
    compass_open: false,
    project_downloading: false,
    update_notification: null,
  };
}
