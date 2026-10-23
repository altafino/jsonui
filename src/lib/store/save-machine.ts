// Save feedback state machine (DD6). Pure reducer; timers live in the store.
import type { CoreError } from "../types/generated/CoreError";

export type SaveState =
  | { phase: "idle" }
  | { phase: "saving"; spinner: boolean }
  | { phase: "saved"; at: string; backup: boolean; backupId: string | null; restored?: boolean }
  | { phase: "settled" }
  | { phase: "error"; error: CoreError };

export type SaveEvent =
  | { type: "START" }
  | { type: "SPIN" }
  | { type: "SUCCESS"; at: string; backupId: string | null; restored?: boolean }
  | { type: "SETTLE" }
  | { type: "FAIL"; error: CoreError }
  | { type: "CANCEL" }
  | { type: "DISMISS" }
  | { type: "EDIT" };

export const SPINNER_DELAY_MS = 150;
export const SAVED_VISIBLE_MS = 8000;

export function saveReducer(state: SaveState, ev: SaveEvent): SaveState {
  switch (ev.type) {
    case "START":
      return { phase: "saving", spinner: false };
    case "SPIN":
      return state.phase === "saving" ? { phase: "saving", spinner: true } : state;
    case "SUCCESS":
      return { phase: "saved", at: ev.at, backup: ev.backupId !== null, backupId: ev.backupId, restored: ev.restored };
    case "SETTLE":
      return state.phase === "saved" ? { phase: "settled" } : state;
    case "FAIL":
      return { phase: "error", error: ev.error };
    case "CANCEL":
      return state.phase === "saving" ? { phase: "idle" } : state;
    case "DISMISS":
      return state.phase === "error" ? { phase: "idle" } : state;
    case "EDIT":
      // A new edit ends the success message but keeps a failure visible until resolved.
      return state.phase === "saved" || state.phase === "settled" ? { phase: "idle" } : state;
  }
}

export const formatClock = (iso: string) => {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

export interface SaveGate {
  enabled: boolean;
  /** Visible reason, also used as tooltip (DD2). */
  reason: string | null;
}

export function saveGate(i: { dirty: number; errors: number; readOnly: boolean; conflict: boolean; editable: boolean; saving: boolean }): SaveGate {
  if (!i.editable) return { enabled: false, reason: "Fix the file's syntax errors to save" };
  if (i.readOnly) return { enabled: false, reason: "This file is read-only" };
  if (i.conflict) return { enabled: false, reason: "Resolve the outside change to save" };
  if (i.saving) return { enabled: false, reason: "Saving" };
  if (i.errors > 0) return { enabled: false, reason: `Fix ${i.errors} error${i.errors === 1 ? "" : "s"} to save` };
  if (i.dirty === 0) return { enabled: false, reason: "No unsaved changes" };
  return { enabled: true, reason: null };
}
