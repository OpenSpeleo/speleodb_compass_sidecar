import { useSyncExternalStore } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { controller } from "./controller";
import type { UiState } from "./types";

/** Event key for UI state notifications; it must match the backend. */
const UI_STATE_EVENT = "ui-state-update";

export const INITIAL_UI_STATE: UiState = {
  loading_state: "NotStarted",
  platform: "Linux",
  user_email: null,
  project_status: [],
  selected_project_id: null,
  compass_open: false,
  project_downloading: false,
  update_notification: null,
};

/** One backend subscription per WebView, independent of React effect replay. */
export function createUiStateStore() {
  let state = INITIAL_UI_STATE;
  let startPromise: Promise<void> | undefined;
  let unlisten: UnlistenFn | undefined;
  let generation = 0;
  const subscribers = new Set<() => void>();
  return {
    getSnapshot: () => state,
    subscribe: (subscriber: () => void) => {
      subscribers.add(subscriber);
      return () => {
        subscribers.delete(subscriber);
      };
    },
    start() {
      if (startPromise) return startPromise;
      const currentGeneration = generation;
      startPromise = (async () => {
        // Subscribe before initialization so its first state event cannot be lost.
        const dispose = await listen<UiState>(UI_STATE_EVENT, (event) => {
          if (generation !== currentGeneration) return;
          state = event.payload;
          subscribers.forEach((subscriber) => subscriber());
        });
        if (generation !== currentGeneration) {
          dispose();
          return;
        }
        unlisten = dispose;
        // Windows' native WebView readiness gate depends on this ordering.
        await controller.ensureInitialized();
      })();
      return startPromise;
    },
    stop() {
      generation++;
      unlisten?.();
      unlisten = undefined;
      startPromise = undefined;
    },
  };
}

export const uiStateStore = createUiStateStore();
export function useUiState(): UiState {
  return useSyncExternalStore(uiStateStore.subscribe, uiStateStore.getSnapshot);
}
if (import.meta.hot) import.meta.hot.dispose(() => uiStateStore.stop());
