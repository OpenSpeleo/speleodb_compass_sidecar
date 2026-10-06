import type { Page } from "@playwright/test";
import type { UiState, ProjectStatus } from "../src/lib/types";

export const projectFixture = (
  status: ProjectStatus["local_status"] = "UpToDate",
): ProjectStatus => ({
  local_status: status,
  info: {
    id: "00000000-0000-0000-0000-000000000001",
    name: "Crystal Cave",
    description: "A cave survey",
    is_active: true,
    permission: "ADMIN",
    active_mutex: null,
    country: "US",
    created_by: "surveyor@example.com",
    creation_date: "2026-01-01T12:00:00Z",
    modified_date: "2026-01-02T12:00:00Z",
    fork_from: null,
    visibility: "PRIVATE",
    exclude_geojson: false,
    latest_commit: null,
    type: "COMPASS",
  },
});
export const stateFixture = (overrides: Partial<UiState> = {}): UiState => ({
  loading_state: "Ready",
  platform: "MacOS",
  user_email: "surveyor@example.com",
  project_status: [projectFixture()],
  selected_project_id: null,
  compass_open: false,
  project_downloading: false,
  update_notification: null,
  ...overrides,
});

/** Supply deterministic browser fixtures at the native Tauri IPC boundary. */
export async function installIpcFixture(
  page: Page,
  state: UiState,
  responses: Record<string, unknown> = {},
) {
  await page.addInitScript(
    ({ initialState, commandResponses }) => {
      let state = initialState;
      let nextId = 1;
      const callbacks = new Map<number, (value: unknown) => void>();
      const listeners = new Map<number, { event: string; handler: number }>();
      const calls: { command: string; args: Record<string, unknown> }[] = [];
      const emit = (event: string, payload: unknown) => {
        for (const [id, listener] of listeners)
          if (listener.event === event)
            callbacks.get(listener.handler)?.({ event, id, payload });
      };
      const invoke = async (
        command: string,
        args: Record<string, unknown> = {},
      ) => {
        calls.push({ command, args });
        if (command === "plugin:event|listen") {
          const id = nextId++;
          listeners.set(id, {
            event: String(args.event),
            handler: Number(args.handler),
          });
          return id;
        }
        if (command === "plugin:event|unlisten") {
          listeners.delete(Number(args.eventId));
          return;
        }
        if (command === "ensure_initialized") {
          emit("ui-state-update", state);
          return;
        }
        if (command === "set_active_project") {
          state = { ...state, selected_project_id: String(args.projectId) };
          emit("ui-state-update", state);
          return;
        }
        if (command === "clear_active_project") {
          state = { ...state, selected_project_id: null };
          emit("ui-state-update", state);
          return;
        }
        if (command === "dismiss_update_notification") {
          state = { ...state, update_notification: null };
          emit("ui-state-update", state);
          return;
        }
        if (command === "about_info")
          return {
            version: "26.9.23",
            authors: ["Jonathan Dekhtiar", "Zachary Heylmun"],
            repo: "https://github.com/OpenSpeleo/speleodb_compass_sidecar",
            description: "Companion app to use SpeleoDB with Compass",
          };
        if (command in commandResponses) {
          const result = commandResponses[command];
          if (result && typeof result === "object" && "reject" in result)
            throw result.reject;
          if (result && typeof result === "object" && "pending" in result)
            return new Promise(() => {});
          return result;
        }
        if (command === "save_project") return "Saved";
        if (command === "pick_compass_project_file") return null;
        return undefined;
      };
      const internals = {
        invoke,
        transformCallback: (
          callback: (value: unknown) => void,
          once = false,
        ) => {
          const id = nextId++;
          callbacks.set(id, (value) => {
            callback(value);
            if (once) callbacks.delete(id);
          });
          return id;
        },
        unregisterCallback: (id: number) => {
          callbacks.delete(id);
        },
        metadata: {
          currentWindow: { label: "main" },
          currentWebview: { label: "main" },
        },
      };
      Object.assign(window, {
        __TAURI_INTERNALS__: internals,
        __TAURI__: { core: { invoke } },
        __TAURI_EVENT_PLUGIN_INTERNALS__: {
          unregisterListener: (event: string, id: number) => {
            listeners.delete(id);
          },
        },
        __SIDECAR_FIXTURE__: {
          calls,
          emitState: (next: typeof state) => {
            state = next;
            emit("ui-state-update", state);
          },
        },
      });
    },
    { initialState: state, commandResponses: responses },
  );
}
