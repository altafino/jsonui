import { X } from "lucide-react";
import { removeAt, toggleSelected, useDocs } from "../../lib/store/doc-store";
import { Tip } from "../ui/Tooltip";
import { useEnv } from "./context";

/** Selection mode is on for this file and editing is allowed. */
export function useSelecting(): boolean {
  const { fileId, frozen } = useEnv();
  return useDocs((s) => !!s.docs[fileId]?.selecting) && !frozen;
}

export const PROTECTED_REASON = "Required, can't be removed";

/** Always-visible muted remove button (hidden in selection mode). Nothing for rows that can't go. */
export function RowRemoval({ pointer, label, removable, isProtected }: { pointer: string; label: string; removable: boolean; isProtected?: boolean }) {
  const { fileId, frozen } = useEnv();
  const selecting = useSelecting();
  if (!removable || isProtected || selecting) return null;
  return (
    <Tip content={`Remove ${label}`}>
      <button type="button" disabled={frozen} aria-label={`Remove ${label}`} onClick={() => void removeAt(fileId, pointer)} className="grid size-8 shrink-0 place-items-center rounded-md text-muted hover:bg-hover hover:text-fg disabled:opacity-50">
        <X className="size-4" aria-hidden />
      </button>
    </Tip>
  );
}

export function SelectBox({ selected, label, onChange }: { selected: boolean; label: string; onChange: () => void }) {
  return (
    <span className="grid size-8 shrink-0 place-items-center">
      <input type="checkbox" checked={selected} onChange={onChange} aria-label={`Select ${label}`} className="size-4 accent-[var(--accent)]" />
    </span>
  );
}

/** Selection checkbox, shown only in selection mode. */
export function ItemSelect({ pointer, label, isProtected, removable = true }: { pointer: string; label: string; isProtected?: boolean; removable?: boolean }) {
  const { fileId } = useEnv();
  const selecting = useSelecting();
  const selected = useDocs((s) => !!s.docs[fileId]?.selected.includes(pointer));
  if (!selecting || isProtected || !removable) return null;
  return <SelectBox selected={selected} label={label} onChange={() => toggleSelected(fileId, pointer)} />;
}
