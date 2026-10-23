// Per-document view state (separate from the app/registry store). Rust owns the draft (R4):
// this store sends ops and applies returned changes to its pointer-keyed node cache (R13).
import { create } from "zustand";
import { ancestorsOf, childPointer, parsePointer } from "../json/pointer";
import { api, toCoreError } from "../tauri-api";
import type { CoreError } from "../types/generated/CoreError";
import type { Diagnostic } from "../types/generated/Diagnostic";
import type { DocKind } from "../types/generated/DocKind";
import type { DocumentSnapshot } from "../types/generated/DocumentSnapshot";
import type { DraftOp } from "../types/generated/DraftOp";
import type { EditResult } from "../types/generated/EditResult";
import type { FieldHint } from "../types/generated/FieldHint";
import type { PresetInfo } from "../types/generated/PresetInfo";
import type { SemanticChange } from "../types/generated/SemanticChange";
import type { SourceView } from "../types/generated/SourceView";
import { useApp } from "./app-store";
import { applyEditResult, isContainer, putTree, type NodeCache } from "./cache";
import { SAVED_VISIBLE_MS, SPINNER_DELAY_MS, formatClock, saveReducer, type SaveEvent, type SaveState } from "./save-machine";

export interface DocState {
  fileId: string;
  phase: "loading" | "ready" | "error" | "missing";
  loadError: CoreError | null;
  kind: DocKind;
  fingerprint: string;
  sizeBytes: number;
  editable: boolean;
  readOnly: boolean;
  sourceView: SourceView | null;
  syncDiagnostics: Diagnostic[];
  schemaDiagnostics: Diagnostic[];
  revision: number;
  dirtyCount: number;
  canUndo: boolean;
  canRedo: boolean;
  nodes: NodeCache;
  fullyLoaded: boolean;
  hints: Record<string, FieldHint>;
  presetInfo: PresetInfo | null;
  expanded: Record<string, true>;
  drillRoot: string;
  focusPointer: string | null;
  query: string;
  changedOnly: boolean;
  showAdvanced: boolean;
  /** Selection mode for removing several settings at once. */
  selecting: boolean;
  selected: string[];
  conflict: boolean;
  save: SaveState;
  opError: CoreError | null;
  /** Draft changes from preview_draft: gives "was <old>" and the Review list. */
  changes: SemanticChange[];
  leftoverTemp: string[];
  externalNote: string | null;
}

const blankDoc = (fileId: string): DocState => ({
  fileId, phase: "loading", loadError: null, kind: "json", fingerprint: "", sizeBytes: 0, editable: false, readOnly: false, sourceView: null,
  syncDiagnostics: [], schemaDiagnostics: [], revision: 0, dirtyCount: 0, canUndo: false, canRedo: false, nodes: {}, fullyLoaded: false,
  hints: {}, presetInfo: null, expanded: {}, drillRoot: "", focusPointer: null, query: "", changedOnly: false, showAdvanced: false, selecting: false, selected: [],
  conflict: false, save: { phase: "idle" }, opError: null, changes: [], leftoverTemp: [], externalNote: null,
});

interface DocsState {
  docs: Record<string, DocState>;
}

export const useDocs = create<DocsState>(() => ({ docs: {} }));

const getDoc = (id: string) => useDocs.getState().docs[id];
export function patch(id: string, p: Partial<DocState> | ((d: DocState) => Partial<DocState>)) {
  useDocs.setState((s) => {
    const cur = s.docs[id];
    if (!cur) return s;
    return { docs: { ...s.docs, [id]: { ...cur, ...(typeof p === "function" ? p(cur) : p) } } };
  });
}

/** Selector hook: `useDoc(id, (d) => d.dirtyCount)`. */
export function useDoc<T>(id: string | null, sel: (d: DocState) => T, fallback: T): T {
  return useDocs((s) => (id && s.docs[id] ? sel(s.docs[id]) : fallback));
}

export const diagnosticsOf = (d: Pick<DocState, "syncDiagnostics" | "schemaDiagnostics">) => [...d.syncDiagnostics, ...d.schemaDiagnostics];
export const countSeverity = (d: Pick<DocState, "syncDiagnostics" | "schemaDiagnostics">, sev: Diagnostic["severity"]) =>
  diagnosticsOf(d).filter((x) => x.severity === sev).length;

// ------------------------------------------------------------------ loading

function fromSnapshot(prev: DocState | undefined, snap: DocumentSnapshot, keepUi: boolean): DocState {
  const base = keepUi && prev ? prev : blankDoc(snap.fileId);
  const nodes: NodeCache = {};
  if (snap.root) putTree(nodes, snap.root);
  return {
    ...base,
    phase: "ready", loadError: null, kind: snap.kind, fingerprint: snap.fingerprint, sizeBytes: Number(snap.sizeBytes),
    editable: snap.editable, readOnly: snap.readOnly, sourceView: snap.sourceView,
    syncDiagnostics: snap.diagnostics, schemaDiagnostics: [], revision: Number(snap.revision), dirtyCount: snap.dirtyCount,
    canUndo: snap.canUndo, canRedo: snap.canRedo, nodes, fullyLoaded: false, hints: {}, conflict: false, opError: null, changes: [], selecting: false, selected: [],
  };
}

const inflight = new Set<string>();

export async function ensureLoaded(fileId: string, pointer: string): Promise<void> {
  const d = getDoc(fileId);
  const n = d?.nodes[pointer];
  if (!d || !n || !isContainer(n) || n.childPointers) return;
  if (!n.childCount) {
    patch(fileId, (x) => ({ nodes: { ...x.nodes, [pointer]: { ...x.nodes[pointer], childPointers: [] } } }));
    return;
  }
  const key = fileId + "\0" + pointer;
  if (inflight.has(key)) return;
  inflight.add(key);
  try {
    const rev = d.revision;
    const tree = await api.getSubtree(fileId, pointer, n.kind === "array" ? 2 : 1);
    patch(fileId, (x) => {
      const nodes = { ...x.nodes };
      putTree(nodes, tree);
      return { nodes };
    });
    if (getDoc(fileId)?.revision !== rev) {
      inflight.delete(key);
      return ensureLoaded(fileId, pointer);
    }
  } catch (e) {
    patch(fileId, { opError: toCoreError(e) });
  } finally {
    inflight.delete(key);
  }
}

export async function ensureFullyLoaded(fileId: string): Promise<void> {
  const d = getDoc(fileId);
  if (!d || d.fullyLoaded || !d.editable) return;
  const tree = await api.getSubtree(fileId, "", 64);
  patch(fileId, (x) => {
    const nodes = { ...x.nodes };
    putTree(nodes, tree);
    return { nodes, fullyLoaded: true };
  });
}

const hintQueue = new Map<string, Set<string>>();
const hintInflight = new Set<string>();
export function requestHints(fileId: string, pointers: string[]) {
  const d = getDoc(fileId);
  if (!d) return;
  let q = hintQueue.get(fileId);
  if (!q) hintQueue.set(fileId, (q = new Set()));
  let added = false;
  for (const p of pointers) {
    const k = fileId + "\0" + p;
    if (d.hints[p] || hintInflight.has(k)) continue;
    q.add(p);
    added = true;
  }
  if (!added) return;
  setTimeout(async () => {
    const batch = [...(hintQueue.get(fileId) ?? [])];
    hintQueue.get(fileId)?.clear();
    if (!batch.length) return;
    batch.forEach((p) => hintInflight.add(fileId + "\0" + p));
    try {
      const hs = await api.getFieldHints(fileId, batch);
      patch(fileId, (x) => ({ hints: { ...x.hints, ...Object.fromEntries(hs.map((h) => [h.pointer, h])) } }));
    } catch {
      /* hints are optional */
    } finally {
      batch.forEach((p) => hintInflight.delete(fileId + "\0" + p));
    }
  }, 0);
}

async function loadExpanded(fileId: string) {
  const d = getDoc(fileId);
  if (!d) return;
  const ptrs = Object.keys(d.expanded).sort((a, b) => a.length - b.length);
  for (const p of ["", d.drillRoot, ...ptrs]) await ensureLoaded(fileId, p);
}

async function defaultExpand(fileId: string) {
  const d = getDoc(fileId);
  const root = d?.nodes[""];
  if (!d || !root?.childPointers) return;
  const exp: Record<string, true> = { ...d.expanded };
  for (const p of root.childPointers) {
    const n = d.nodes[p];
    if (!isContainer(n)) continue;
    if (n.kind === "object") exp[p] = true;
    else {
      patch(fileId, { expanded: { ...exp } });
      await ensureLoaded(fileId, p);
      const first = getDoc(fileId)?.nodes[p]?.childPointers?.[0];
      const fk = first ? getDoc(fileId)?.nodes[first]?.kind : null;
      if (!first || (fk && !isContainer({ kind: fk }))) exp[p] = true;
      else delete exp[p];
    }
  }
  patch(fileId, { expanded: exp });
  await loadExpanded(fileId);
}

export async function openDoc(fileId: string, opts: { force?: boolean } = {}): Promise<void> {
  const existing = getDoc(fileId);
  if (existing && existing.phase === "ready" && !opts.force) return;
  useDocs.setState((s) => ({ docs: { ...s.docs, [fileId]: { ...(s.docs[fileId] ?? blankDoc(fileId)), phase: "loading" } } }));
  try {
    const snap = await api.openDocument(fileId);
    useDocs.setState((s) => ({ docs: { ...s.docs, [fileId]: fromSnapshot(s.docs[fileId], snap, true) } }));
    const [preset, leftover] = await Promise.all([api.getPresetInfo(fileId).catch(() => null), api.listLeftoverTempFiles(fileId).catch(() => [])]);
    patch(fileId, { presetInfo: preset, leftoverTemp: leftover });
    if (snap.editable) {
      requestHints(fileId, [""]);
      await defaultExpand(fileId);
    }
  } catch (e) {
    const err = toCoreError(e);
    patch(fileId, { phase: err.code === "NOT_FOUND" ? "missing" : "error", loadError: err });
  }
}

export function closeDoc(fileId: string) {
  useDocs.setState((s) => {
    const { [fileId]: _gone, ...rest } = s.docs;
    void _gone;
    return { docs: rest };
  });
}

async function adoptSnapshot(fileId: string, snap: DocumentSnapshot) {
  useDocs.setState((s) => ({ docs: { ...s.docs, [fileId]: fromSnapshot(s.docs[fileId], snap, true) } }));
  void refreshChanges(fileId);
  const d = getDoc(fileId)!;
  if (d.editable) {
    requestHints(fileId, [""]);
    await loadExpanded(fileId);
  }
}

export async function reloadDoc(fileId: string, note: string | null = null) {
  try {
    const snap = await api.reloadDocument(fileId);
    await adoptSnapshot(fileId, snap);
    patch(fileId, { externalNote: note });
  } catch (e) {
    patch(fileId, { opError: toCoreError(e) });
  }
}

export async function discardDraft(fileId: string) {
  try {
    const snap = await api.discardDraft(fileId);
    await adoptSnapshot(fileId, snap);
    useApp.getState().announce("Changes discarded");
  } catch (e) {
    patch(fileId, { opError: toCoreError(e) });
  }
}

// ------------------------------------------------------------------ edits

function absorb(fileId: string, res: EditResult) {
  patch(fileId, (d) => ({
    nodes: applyEditResult(d.nodes, res),
    fullyLoaded: d.fullyLoaded && res.reshaped.length === 0,
    hints: Object.fromEntries(Object.entries(d.hints).filter(([p]) => !res.reshaped.includes(p))),
    revision: Number(res.revision), dirtyCount: res.dirtyCount, canUndo: res.canUndo, canRedo: res.canRedo,
    syncDiagnostics: res.diagnostics, opError: null,
    save: saveReducer(d.save, { type: "EDIT" }),
  }));
  void refreshChanges(fileId);
  // Containers whose descendants were dropped reload lazily where expanded.
  void loadExpanded(fileId);
}

let changesTimer: ReturnType<typeof setTimeout> | undefined;
export function refreshChanges(fileId: string) {
  clearTimeout(changesTimer);
  const d = getDoc(fileId);
  if (!d || d.dirtyCount === 0) {
    patch(fileId, { changes: [] });
    return Promise.resolve();
  }
  return new Promise<void>((resolve) => {
    changesTimer = setTimeout(async () => {
      try {
        const sum = await api.previewDraft(fileId);
        patch(fileId, { changes: sum.changes });
      } catch {
        /* advisory */
      }
      resolve();
    }, 60);
  });
}

/** Edits in flight; Save and document switches wait for them (typed value + Ctrl+S). */
const pending = new Set<Promise<unknown>>();
export async function flushEdits() {
  // Commit a value still being typed (blur triggers the field's commit); leave buttons focused.
  const el = document.activeElement as HTMLElement | null;
  if (el && /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName)) el.blur();
  await Promise.resolve();
  await Promise.allSettled([...pending]);
}

async function run(fileId: string, fn: () => Promise<EditResult>): Promise<boolean> {
  const p = runInner(fileId, fn);
  pending.add(p);
  void p.finally(() => pending.delete(p));
  return p;
}

async function runInner(fileId: string, fn: () => Promise<EditResult>): Promise<boolean> {
  try {
    absorb(fileId, await fn());
    return true;
  } catch (e) {
    const err = toCoreError(e);
    if (err.code === "EXTERNAL_CHANGE_CONFLICT") patch(fileId, { conflict: true });
    patch(fileId, { opError: err });
    useApp.getState().announce(err.message, "assertive");
    return false;
  }
}

export const applyOp = (fileId: string, op: DraftOp) => run(fileId, () => api.applyDraftOperation(fileId, op));
export const undo = (fileId: string) => getDoc(fileId)?.canUndo ? run(fileId, () => api.undoDraftOperation(fileId)) : Promise.resolve(false);
export const redo = (fileId: string) => getDoc(fileId)?.canRedo ? run(fileId, () => api.redoDraftOperation(fileId)) : Promise.resolve(false);
export const revertChange = (fileId: string, opIndex: number) => run(fileId, () => api.revertChange(fileId, opIndex));
export const reapply = (fileId: string) => run(fileId, async () => {
  const r = await api.reapplyDraft(fileId);
  patch(fileId, { conflict: false });
  // The base changed under the draft: take the new fingerprint.
  const snap = await api.openDocument(fileId);
  patch(fileId, { fingerprint: snap.fingerprint });
  return r;
});

export const setValue = (fileId: string, pointer: string, valueJson: string) => applyOp(fileId, { op: "set", path: pointer, valueJson });
export const addMember = (fileId: string, parent: string, key: string, valueJson: string) => applyOp(fileId, { op: "add", path: childPointer(parent, key), valueJson });
export const removeDraftItems = (fileId: string, pointers: string[]) => run(fileId, () => api.removeDraftItems(fileId, pointers));
export const removeAt = (fileId: string, pointer: string) => applyOp(fileId, { op: "remove", path: pointer });
export const insertItem = (fileId: string, array: string, index: number, valueJson: string) => applyOp(fileId, { op: "array-insert", path: array, index, valueJson });
export const moveItem = (fileId: string, array: string, from: number, to: number) => applyOp(fileId, { op: "array-move", path: array, from, to });
export const renameKey = (fileId: string, pointer: string, newKey: string) => applyOp(fileId, { op: "rename-key", path: pointer, newKey });

// ------------------------------------------------------------------ diagnostics events (R14)

/** Drops results for any revision other than the current draft revision. */
export function receiveDiagnostics(fileId: string, revision: number | bigint, diagnostics: Diagnostic[]): boolean {
  const d = getDoc(fileId);
  if (!d || Number(revision) !== d.revision) return false;
  patch(fileId, { schemaDiagnostics: diagnostics });
  return true;
}

// ------------------------------------------------------------------ UI state

export const setQuery = (id: string, query: string) => patch(id, { query });
export const setChangedOnly = (id: string, changedOnly: boolean) => patch(id, { changedOnly });
export const setSelecting = (id: string, selecting: boolean) => patch(id, { selecting, selected: [] });
export const toggleSelected = (id: string, pointer: string) =>
  patch(id, (d) => ({ selected: d.selected.includes(pointer) ? d.selected.filter((p) => p !== pointer) : [...d.selected, pointer] }));
/** Sends the raw selection; the backend dedupes, drops descendants and orders array removals. */
export async function removeSelected(fileId: string) {
  const pointers = getDoc(fileId)?.selected ?? [];
  if (!pointers.length || !(await removeDraftItems(fileId, pointers))) return;
  setSelecting(fileId, false);
  useApp.getState().announce(`Removed ${pointers.length} ${pointers.length === 1 ? "item" : "items"}`);
}
export const setShowAdvanced = (id: string, showAdvanced: boolean) => patch(id, { showAdvanced });

export function toggleExpanded(fileId: string, pointer: string, open?: boolean) {
  patch(fileId, (d) => {
    const expanded = { ...d.expanded };
    const want = open ?? !expanded[pointer];
    if (want) expanded[pointer] = true;
    else delete expanded[pointer];
    return { expanded };
  });
  if (open ?? true) void ensureLoaded(fileId, pointer);
}

export function drillInto(fileId: string, pointer: string) {
  patch(fileId, { drillRoot: pointer, focusPointer: null });
  void ensureLoaded(fileId, pointer);
}

/** Drill-in rule (DD3): a container at nesting level >= 4 below the drill root opens as its own view. */
export const MAX_INLINE_LEVEL = 3;

/** Makes `pointer` visible: expands ancestors, picks a drill root so it is within inline depth, focuses it. */
export async function revealPointer(fileId: string, pointer: string) {
  const toks = parsePointer(pointer);
  const rootLen = Math.max(0, toks.length - MAX_INLINE_LEVEL);
  const drillRoot = "/" + toks.slice(0, rootLen).map((t) => t.replace(/~/g, "~0").replace(/\//g, "~1")).join("/");
  const anc = ancestorsOf(pointer).filter((a) => a !== "");
  patch(fileId, (d) => {
    const expanded = { ...d.expanded };
    for (const a of anc) expanded[a] = true;
    return { expanded, drillRoot: rootLen === 0 ? "" : drillRoot, query: "", changedOnly: false };
  });
  for (const a of ["", ...anc]) await ensureLoaded(fileId, a);
  patch(fileId, { focusPointer: pointer });
}

// ------------------------------------------------------------------ save

const saveTimers = new Map<string, ReturnType<typeof setTimeout>[]>();
function clearSaveTimers(fileId: string) {
  saveTimers.get(fileId)?.forEach(clearTimeout);
  saveTimers.set(fileId, []);
}
function sendSave(fileId: string, ev: SaveEvent) {
  patch(fileId, (d) => ({ save: saveReducer(d.save, ev) }));
}
function later(fileId: string, ms: number, fn: () => void) {
  const t = setTimeout(fn, ms);
  saveTimers.set(fileId, [...(saveTimers.get(fileId) ?? []), t]);
}

export interface SaveOptions {
  acceptWarnings?: boolean;
}

export async function saveDocument(fileId: string, opts: SaveOptions = {}): Promise<boolean> {
  await flushEdits();
  const d = getDoc(fileId);
  if (!d) return false;
  const app = useApp.getState();
  clearSaveTimers(fileId);
  sendSave(fileId, { type: "START" });
  later(fileId, SPINNER_DELAY_MS, () => sendSave(fileId, { type: "SPIN" }));
  try {
    const res = await api.saveDraft(fileId, d.fingerprint, !!opts.acceptWarnings);
    clearSaveTimers(fileId);
    const snap = await api.openDocument(fileId);
    await adoptSnapshot(fileId, snap);
    patch(fileId, { fingerprint: res.fingerprint });
    sendSave(fileId, { type: "SUCCESS", at: res.savedAt, backupId: res.backupId });
    app.announce(`Saved ${formatClock(res.savedAt)}${res.backupId ? ", backup created" : ""}`);
    later(fileId, SAVED_VISIBLE_MS, () => sendSave(fileId, { type: "SETTLE" }));
    void app.refreshStatuses();
    return true;
  } catch (e) {
    clearSaveTimers(fileId);
    const err = toCoreError(e);
    switch (err.code) {
      case "EXTERNAL_CHANGE_CONFLICT":
        patch(fileId, { conflict: true });
        sendSave(fileId, { type: "CANCEL" });
        app.openDialog({ kind: "conflict", fileId });
        break;
      case "PRESET_WARNINGS_REQUIRE_CONFIRMATION":
        sendSave(fileId, { type: "CANCEL" });
        app.openDialog({ kind: "review", fileId });
        break;
      case "SYMLINK_REQUIRES_CONFIRMATION":
      case "HARDLINK_REQUIRES_CONFIRMATION":
        sendSave(fileId, { type: "CANCEL" });
        app.openDialog({ kind: "link", fileId, code: err.code, resume: "save" });
        break;
      default:
        sendSave(fileId, { type: "FAIL", error: err });
        app.announce(`Save failed. ${err.message}`, "assertive");
    }
    return false;
  }
}

export const dismissSaveError = (fileId: string) => sendSave(fileId, { type: "DISMISS" });

/** "Undo save": restores the backup made by the last save through the normal restore flow. */
export async function undoSave(fileId: string): Promise<boolean> {
  const d = getDoc(fileId);
  if (!d || d.save.phase !== "saved" || !d.save.backupId) return false;
  const app = useApp.getState();
  const backupId = d.save.backupId;
  clearSaveTimers(fileId);
  sendSave(fileId, { type: "START" });
  later(fileId, SPINNER_DELAY_MS, () => sendSave(fileId, { type: "SPIN" }));
  try {
    const res = await api.restoreBackup(fileId, backupId, d.fingerprint);
    clearSaveTimers(fileId);
    const snap = await api.openDocument(fileId);
    await adoptSnapshot(fileId, snap);
    sendSave(fileId, { type: "SUCCESS", at: res.savedAt, backupId: null, restored: true });
    app.announce("Previous version restored");
    later(fileId, SAVED_VISIBLE_MS, () => sendSave(fileId, { type: "SETTLE" }));
    return true;
  } catch (e) {
    clearSaveTimers(fileId);
    const err = toCoreError(e);
    if (err.code === "EXTERNAL_CHANGE_CONFLICT") patch(fileId, { conflict: true });
    sendSave(fileId, { type: "FAIL", error: err });
    app.announce(`Restore failed. ${err.message}`, "assertive");
    return false;
  }
}

export async function restoreFromHistory(fileId: string, backupId: string): Promise<boolean> {
  const d = getDoc(fileId);
  if (!d) return false;
  try {
    const res = await api.restoreBackup(fileId, backupId, d.fingerprint);
    const snap = await api.openDocument(fileId);
    await adoptSnapshot(fileId, snap);
    sendSave(fileId, { type: "SUCCESS", at: res.savedAt, backupId: null, restored: true });
    useApp.getState().announce("Backup restored");
    later(fileId, SAVED_VISIBLE_MS, () => sendSave(fileId, { type: "SETTLE" }));
    return true;
  } catch (e) {
    const err = toCoreError(e);
    if (err.code === "EXTERNAL_CHANGE_CONFLICT") {
      patch(fileId, { conflict: true });
      useApp.getState().openDialog({ kind: "conflict", fileId });
    } else sendSave(fileId, { type: "FAIL", error: err });
    return false;
  }
}

export async function confirmLinkAndSave(fileId: string): Promise<boolean> {
  await api.confirmLinkWrite(fileId);
  await useApp.getState().refreshFiles();
  useApp.getState().closeDialog();
  return saveDocument(fileId);
}

export function externalChange(fileId: string, hasUnsavedChanges: boolean) {
  const d = getDoc(fileId);
  if (!d) return;
  if (hasUnsavedChanges || d.dirtyCount > 0) patch(fileId, { conflict: true });
  else void reloadDoc(fileId, "Reloaded: this file changed outside ConfigDeck.");
}

export const _internal = { blankDoc };
