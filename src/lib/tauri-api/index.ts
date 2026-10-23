import type { Api, Events, MockControls } from "./types";

export * from "./types";

let impl: Api | null = null;
let evImpl: Events | null = null;
let controls: MockControls | null = null;

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Picks the backend once at startup: the real Tauri IPC when running inside the shell,
 * otherwise the in-memory mock (dev in a browser, Playwright, Vitest). The mock is a
 * separate dynamic chunk and is never imported on the Tauri path.
 */
export async function initApi(): Promise<void> {
  if (impl) return;
  if (isTauri()) {
    const real = await import("./real");
    impl = real.realApi;
    evImpl = real.realEvents;
  } else {
    const mock = await import("./mock");
    const m = mock.createMockBackend();
    impl = m.api;
    evImpl = m.events;
    controls = m.controls;
    (window as unknown as { __configdeckMock?: MockControls }).__configdeckMock = m.controls;
  }
}

/** Test helper: install a specific implementation. */
export function setApiForTests(a: Api, e: Events, c: MockControls | null = null) {
  impl = a;
  evImpl = e;
  controls = c;
}

const need = () => {
  if (!impl) throw new Error("API not initialised: call initApi() first");
  return impl;
};

export const api: Api = new Proxy({} as Api, {
  get: (_t, name: string) => (...args: unknown[]) => (need() as unknown as Record<string, (...a: unknown[]) => unknown>)[name](...args),
});

export const events: Events = {
  listen: (event, handler) => {
    if (!evImpl) throw new Error("API not initialised: call initApi() first");
    return evImpl.listen(event, handler);
  },
};

export const mockControls = (): MockControls | null => controls;
