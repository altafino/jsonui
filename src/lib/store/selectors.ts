import { useApp } from "./app-store";
import { countSeverity, saveDocument, useDocs } from "./doc-store";
import { saveGate, type SaveGate } from "./save-machine";

export interface SaveInfo {
  fileId: string | null;
  dirty: number;
  errors: number;
  warnings: number;
  gate: SaveGate;
}

const NONE: SaveInfo = { fileId: null, dirty: 0, errors: 0, warnings: 0, gate: { enabled: false, reason: "No unsaved changes" } };

export function useSaveInfo(): SaveInfo {
  const fileId = useApp((s) => s.activeFileId);
  const doc = useDocs((s) => (fileId ? s.docs[fileId] : undefined));
  if (!fileId || !doc || doc.phase !== "ready") return NONE;
  const errors = countSeverity(doc, "error");
  const warnings = countSeverity(doc, "warning");
  return {
    fileId, dirty: doc.dirtyCount, errors, warnings,
    gate: saveGate({ dirty: doc.dirtyCount, errors, readOnly: doc.readOnly, conflict: doc.conflict, editable: doc.editable, saving: doc.save.phase === "saving" }),
  };
}

/** Save entry point shared by the footer button and Ctrl+S: warnings go through the Review confirm (eng R2). */
export function requestSave(): void {
  const app = useApp.getState();
  const id = app.activeFileId;
  if (!id) return;
  const doc = useDocs.getState().docs[id];
  if (!doc || doc.phase !== "ready") return;
  const errors = countSeverity(doc, "error");
  const gate = saveGate({ dirty: doc.dirtyCount, errors, readOnly: doc.readOnly, conflict: doc.conflict, editable: doc.editable, saving: doc.save.phase === "saving" });
  if (!gate.enabled) {
    if (gate.reason) app.announce(gate.reason);
    return;
  }
  if (countSeverity(doc, "warning") > 0) app.openDialog({ kind: "review", fileId: id });
  else void saveDocument(id);
}
