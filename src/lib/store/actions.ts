// App-level workflows that touch both stores (selection guards, registry actions, startup).
import { events, initApi, api, toCoreError } from "../tauri-api";
import type { ManagedDocument } from "../types/generated/ManagedDocument";
import { useApp } from "./app-store";
import { closeDoc, discardDraft, externalChange, openDoc, patch, receiveDiagnostics, saveDocument, useDocs } from "./doc-store";


const dirtyOf = (id: string | null) => (id ? (useDocs.getState().docs[id]?.dirtyCount ?? 0) : 0);

export async function activate(id: string) {
  useApp.getState().setActive(id);
  useApp.getState().setInfo(null);
  await openDoc(id);
  void useApp.getState().refreshFiles();
}

/** Switching documents with an unsaved draft asks Save / Discard / Cancel (spec §10). */
export function requestSelect(id: string) {
  const app = useApp.getState();
  const from = app.activeFileId;
  if (from === id) return;
  if (from && dirtyOf(from) > 0) app.openDialog({ kind: "unsaved", fromId: from, target: { type: "file", id } });
  else void activate(id);
}

export async function resolveUnsaved(choice: "save" | "discard") {
  const app = useApp.getState();
  const d = app.dialog;
  if (d?.kind !== "unsaved") return;
  const { fromId, target } = d;
  if (choice === "save") {
    app.closeDialog();
    const ok = await saveDocument(fromId);
    if (!ok) return;
  } else {
    await discardDraft(fromId);
    app.closeDialog();
  }
  if (target.type === "file") await activate(target.id);
}

async function addAndSelect(p: Promise<ManagedDocument | null>) {
  try {
    const doc = await p;
    if (!doc) return;
    await useApp.getState().refreshFiles();
    void useApp.getState().refreshStatuses();
    requestSelect(doc.id);
  } catch (e) {
    useApp.getState().announce(toCoreError(e).message, "assertive");
    useApp.setState({ info: toCoreError(e).message });
  }
}

export const addFile = () => addAndSelect(api.pickAndAddManagedFile());
export const createFile = () => addAndSelect(api.pickAndCreateManagedFile());
export const trySample = () => addAndSelect(api.createSampleFile());
export const addDiscovered = (candidateId: string) => addAndSelect(api.addDiscoveredFile(candidateId));
export async function createMissing(fileId: string) {
  try {
    await api.createMissingFile(fileId);
    await useApp.getState().refreshStatuses();
    await openDoc(fileId, { force: true });
  } catch (e) {
    useApp.getState().announce(toCoreError(e).message, "assertive");
  }
}
export const relocate = (fileId: string) => addAndSelect(api.relocateManagedFile(fileId)).then(() => openDoc(fileId, { force: true }));

export async function removeFile(fileId: string) {
  const app = useApp.getState();
  await api.removeManagedFile(fileId);
  closeDoc(fileId);
  await app.refreshFiles();
  const files = useApp.getState().files;
  if (app.activeFileId === fileId) {
    useApp.getState().setActive(null);
    if (files[0]) await activate(files[0].id);
  }
}

export async function renameFile(fileId: string, displayName: string) {
  await api.updateManagedFileMetadata(fileId, { displayName, collectionId: null });
  await useApp.getState().refreshFiles();
}

export async function copyText(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    useApp.getState().announce(`${what} copied`);
  } catch {
    useApp.getState().announce(`Could not copy ${what}`, "assertive");
  }
}

let booting: Promise<void> | null = null;
export const bootstrap = () => (booting ??= boot());

async function boot() {
  await initApi();
  const app = useApp.getState();
  try {
    const [prefs, files, statuses, notices] = await Promise.all([api.getPreferences(), api.listManagedFiles(), api.getFileStatuses(), api.getStartupNotices()]);
    useApp.setState({ prefs, files, notices });
    app.setStatuses(statuses);
  } catch (e) {
    useApp.setState({ fatal: toCoreError(e) });
  }
  void events.listen("managed-file-external-change", (p) => {
    externalChange(p.fileId, p.hasUnsavedChanges);
    void useApp.getState().refreshStatuses();
  });
  void events.listen("managed-file-missing", (p) => {
    patch(p.fileId, { phase: "missing" });
    void useApp.getState().refreshStatuses();
  });
  void events.listen("managed-file-access-changed", () => void useApp.getState().refreshStatuses());
  void events.listen("draft-diagnostics", (p) => {
    receiveDiagnostics(p.fileId, p.revision, p.diagnostics);
  });
  app.setReady();
  const files = useApp.getState().files;
  const last = [...files].sort((a, b) => (b.lastOpenedAt ?? "").localeCompare(a.lastOpenedAt ?? ""))[0] ?? files[0];
  if (last) await activate(last.id);
}

/** Test helper: allows bootstrap() to run again against a fresh mock. */
export function _resetForTests() {
  booting = null;
}
