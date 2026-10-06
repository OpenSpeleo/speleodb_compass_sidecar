import { expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { controller } from "./controller";
import { createUiStateStore, INITIAL_UI_STATE } from "./state";
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("./controller", () => ({ controller: { ensureInitialized: vi.fn() } }));
it("awaits registration before initialization and starts only once", async () => {
  let finish!: (dispose: () => void) => void;
  vi.mocked(listen).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const store = createUiStateStore();
  const first = store.start();
  expect(store.start()).toBe(first);
  expect(controller.ensureInitialized).not.toHaveBeenCalled();
  const dispose = vi.fn();
  finish(dispose);
  await first;
  expect(controller.ensureInitialized).toHaveBeenCalledTimes(1);
  store.stop();
  store.stop();
  expect(dispose).toHaveBeenCalledTimes(1);
});
it("accepts the first initialization event and cleans subscribers independently", async () => {
  const store = createUiStateStore();
  const changed = vi.fn();
  const unsubscribe = store.subscribe(changed);
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    vi.mocked(controller.ensureInitialized).mockImplementation(async () =>
      handler({
        event: "ui-state-update",
        id: 1,
        payload: { ...INITIAL_UI_STATE, loading_state: "Unauthenticated" },
      }),
    );
    return vi.fn<() => void>();
  });
  await store.start();
  expect(store.getSnapshot().loading_state).toBe("Unauthenticated");
  expect(changed).toHaveBeenCalledTimes(1);
  unsubscribe();
  store.stop();
});
it("cleans a subscription that finishes registering after disposal without initializing", async () => {
  let finish!: (dispose: () => void) => void;
  vi.mocked(listen).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const store = createUiStateStore();
  const pending = store.start();
  store.stop();
  const dispose = vi.fn();
  finish(dispose);
  await pending;
  expect(dispose).toHaveBeenCalledOnce();
  expect(controller.ensureInitialized).not.toHaveBeenCalled();
});
