import { useEffect } from "react";
import { FIELD_SEARCH_ID, isTextTarget, matchShortcut } from "../shortcuts";
import { useApp } from "../store/app-store";
import { drillInto, redo, undo, useDocs } from "../store/doc-store";
import { parentPointer } from "../json/pointer";
import { requestSave } from "../store/selectors";

function cycleLandmark(dir: 1 | -1) {
  const marks = [...document.querySelectorAll<HTMLElement>("[data-landmark]")];
  if (!marks.length) return;
  const idx = marks.findIndex((m) => m.contains(document.activeElement));
  const next = marks[(idx + dir + marks.length) % marks.length];
  next.focus();
}

function goBack(): boolean {
  const id = useApp.getState().activeFileId;
  const doc = id ? useDocs.getState().docs[id] : undefined;
  if (!id || !doc || doc.drillRoot === "") return false;
  drillInto(id, parentPointer(doc.drillRoot));
  return true;
}

/** Global keyboard shortcuts (DD14). */
export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const app = useApp.getState();
      if (e.key === "Escape" && !app.dialog && !isTextTarget(e.target) && !(e.target as HTMLElement | null)?.closest?.("[role=menu],[role=listbox]")) {
        if (goBack()) e.preventDefault();
        else if (app.rail && app.sidebarOpen) app.toggleSidebar();
        return;
      }
      const action = matchShortcut(e);
      if (!action) return;
      const id = app.activeFileId;
      switch (action) {
        case "command-search":
          e.preventDefault();
          if (id) app.openDialog({ kind: "command" });
          break;
        case "field-search": {
          const el = document.getElementById(FIELD_SEARCH_ID);
          if (el) (e.preventDefault(), el.focus(), (el as HTMLInputElement).select());
          break;
        }
        case "save":
          e.preventDefault();
          if (!app.dialog) requestSave();
          break;
        case "undo":
          if (id && !app.dialog) (e.preventDefault(), void undo(id));
          break;
        case "redo":
          if (id && !app.dialog) (e.preventDefault(), void redo(id));
          break;
        case "toggle-sidebar":
          e.preventDefault();
          app.toggleSidebar();
          break;
        case "back":
          if (!isTextTarget(e.target) && goBack()) e.preventDefault();
          else if (isTextTarget(e.target) && (e.target as HTMLInputElement).selectionStart === 0 && goBack()) e.preventDefault();
          break;
        case "cycle-landmark":
          e.preventDefault();
          cycleLandmark(1);
          break;
        case "cycle-landmark-back":
          e.preventDefault();
          cycleLandmark(-1);
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
