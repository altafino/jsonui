import { act, render } from "@testing-library/react";
import App from "../App";
import { _resetForTests, activate, bootstrap } from "../lib/store/actions";
import { DEFAULT_PREFS, useApp } from "../lib/store/app-store";
import { useDocs } from "../lib/store/doc-store";
import { setApiForTests } from "../lib/tauri-api";
import { createMockBackendRaw } from "../lib/tauri-api/mock";

export type Mock = ReturnType<typeof createMockBackendRaw>;

/** Fresh mock backend + empty stores. */
export function freshBackend(flags: string[] = ["seeded"], saveDelayMs = 0): Mock {
  const m = createMockBackendRaw({ flags, saveDelayMs });
  setApiForTests(m.api, m.events, m.controls);
  _resetForTests();
  useDocs.setState({ docs: {} });
  useApp.setState({ ready: false, files: [], statuses: {}, notices: [], dismissedNotices: [], prefs: DEFAULT_PREFS, activeFileId: null, dialog: null, info: null, fatal: null, sidebarOpen: true, rail: false });
  return m;
}

/** Boots the whole app against a mock and optionally opens a file by display name. */
export async function bootApp(opts: { flags?: string[]; open?: string; saveDelayMs?: number } = {}) {
  const m = freshBackend(opts.flags ?? ["seeded"], opts.saveDelayMs);
  const utils = render(<App />);
  await act(async () => {
    await bootstrap();
  });
  if (opts.open) {
    const f = useApp.getState().files.find((x) => x.displayName === opts.open);
    if (!f) throw new Error("no such mock file " + opts.open);
    await act(async () => {
      await activate(f.id);
    });
  }
  return { m, ...utils };
}

export const sleep = (ms: number) => act(() => new Promise<void>((r) => setTimeout(r, ms)));
