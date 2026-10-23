import { createMockBackend as create, type MockOptions } from "./backend";

/** Flags come from `?mock=seeded,recovered,...` so Playwright and humans can pick a scenario. */
export function createMockBackend(options?: MockOptions) {
  const fromUrl = typeof location !== "undefined" ? new URLSearchParams(location.search).get("mock") : null;
  const flags = options?.flags ?? (fromUrl ? fromUrl.split(",").map((f) => f.trim()).filter(Boolean) : []);
  return create({ ...options, flags });
}
export { create as createMockBackendRaw };
