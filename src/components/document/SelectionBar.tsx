import { useEffect } from "react";
import { useApp } from "../../lib/store/app-store";
import { removeSelected, setSelecting, useDocs } from "../../lib/store/doc-store";
import { Button } from "../ui/Button";

/** Sticky bar of selection mode: live count, Remove selected, Cancel. Esc leaves selection mode. */
export function SelectionBar({ fileId }: { fileId: string }) {
  const doc = useDocs((s) => s.docs[fileId]);
  const frozen = !doc || doc.readOnly || doc.conflict || !doc.editable;
  const active = !!doc?.selecting && !frozen;

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      // Menus, dialogs and the search box use Esc for themselves first.
      if (e.key !== "Escape" || e.defaultPrevented || useApp.getState().dialog || document.querySelector("[role=menu]")) return;
      setSelecting(fileId, false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, fileId]);

  if (!active) return null;
  const n = doc.selected.length;
  return (
    <div role="region" aria-label="Selection" className="sticky bottom-0 z-10 flex items-center gap-3 border-t border-line bg-sidebar px-4 py-2.5">
      <span role="status" className="text-[0.9375rem] font-medium">{n} selected</span>
      <div className="ml-auto flex items-center gap-2">
        <Button onClick={() => setSelecting(fileId, false)}>Cancel</Button>
        <Button variant="danger" disabled={n === 0} onClick={() => void removeSelected(fileId)}>Remove selected</Button>
      </div>
    </div>
  );
}
