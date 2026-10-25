import { create } from "zustand";
import { api, toCoreError } from "../tauri-api";
import type { CoreError } from "../types/generated/CoreError";
import type { FileStatus } from "../types/generated/FileStatus";
import type { ManagedDocument } from "../types/generated/ManagedDocument";
import type { Notice } from "../types/generated/Notice";
import type { Preferences } from "../types/generated/Preferences";

export type Dialog =
  | { kind: "settings"; tab?: "appearance" | "shortcuts" | "files" }
  | { kind: "about" }
  | { kind: "history"; fileId: string }
  | { kind: "review"; fileId: string }
  | { kind: "conflict"; fileId: string }
  | { kind: "discover" }
  | { kind: "unsaved"; fromId: string; target: { type: "file"; id: string } | { type: "close" } }
  | { kind: "link"; fileId: string; code: "SYMLINK_REQUIRES_CONFIRMATION" | "HARDLINK_REQUIRES_CONFIRMATION"; resume: "save" }
  | { kind: "rename"; fileId: string }
  | { kind: "remove"; fileId: string }
  | { kind: "reload"; fileId: string }
  | { kind: "command" }
  | { kind: "addSetting"; fileId: string; parent: string; name?: string };

export const DEFAULT_PREFS: Preferences = { theme: "system", density: "comfortable", fontScale: 1, backupRetention: 20, hideSecrets: true, sidebarWidth: 320 };

interface AppState {
  ready: boolean;
  files: ManagedDocument[];
  statuses: Record<string, FileStatus>;
  notices: Notice[];
  dismissedNotices: string[];
  prefs: Preferences;
  activeFileId: string | null;
  /** User toggle (Ctrl+B). In rail mode this opens the overlay. */
  sidebarOpen: boolean;
  rail: boolean;
  dialog: Dialog | null;
  /** Non-blocking info line, e.g. "Reloaded after an outside change". */
  info: string | null;
  fatal: CoreError | null;
  announcement: { polite: string; assertive: string; tick: number };

  setReady: () => void;
  setFiles: (files: ManagedDocument[]) => void;
  setStatuses: (s: FileStatus[]) => void;
  refreshFiles: () => Promise<void>;
  refreshStatuses: () => Promise<void>;
  setActive: (id: string | null) => void;
  setRail: (rail: boolean) => void;
  toggleSidebar: () => void;
  openDialog: (d: Dialog) => void;
  closeDialog: () => void;
  setPrefs: (p: Partial<Preferences>) => Promise<void>;
  setInfo: (m: string | null) => void;
  dismissNotice: (code: string) => void;
  announce: (message: string, mode?: "polite" | "assertive") => void;
}

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  files: [],
  statuses: {},
  notices: [],
  dismissedNotices: [],
  prefs: DEFAULT_PREFS,
  activeFileId: null,
  sidebarOpen: true,
  rail: false,
  dialog: null,
  info: null,
  fatal: null,
  announcement: { polite: "", assertive: "", tick: 0 },

  setReady: () => set({ ready: true }),
  setFiles: (files) => set({ files }),
  setStatuses: (s) => set({ statuses: Object.fromEntries(s.map((x) => [x.fileId, x])) }),
  refreshFiles: async () => {
    try {
      set({ files: await api.listManagedFiles() });
    } catch (e) {
      set({ fatal: toCoreError(e) });
    }
  },
  refreshStatuses: async () => {
    try {
      get().setStatuses(await api.getFileStatuses());
    } catch {
      /* statuses are advisory */
    }
  },
  setActive: (id) => set({ activeFileId: id }),
  setRail: (rail) => set({ rail, sidebarOpen: rail ? false : true }),
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: null }),
  setPrefs: async (p) => {
    const prefs = { ...get().prefs, ...p };
    set({ prefs });
    try {
      await api.setPreferences(prefs);
    } catch (e) {
      set({ fatal: toCoreError(e) });
    }
  },
  setInfo: (info) => set({ info }),
  dismissNotice: (code) => set((s) => ({ dismissedNotices: [...s.dismissedNotices, code] })),
  announce: (message, mode = "polite") =>
    set((s) => ({
      announcement: { polite: mode === "polite" ? message : s.announcement.polite, assertive: mode === "assertive" ? message : s.announcement.assertive, tick: s.announcement.tick + 1 },
    })),
}));
