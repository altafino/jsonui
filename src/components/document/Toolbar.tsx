import clsx from "clsx";
import { Redo2, Search, Undo2, X } from "lucide-react";
import { FIELD_SEARCH_ID } from "../../lib/shortcuts";
import { drillTarget } from "./drill";
import { redo, setChangedOnly, setQuery, setSelecting, setShowAdvanced, undo, useDocs } from "../../lib/store/doc-store";
import { AddSettingMenu } from "../fields/Containers";
import { FieldEnvContext } from "../fields/context";
import { IconButton } from "../ui/Button";

function Toggle({ pressed, onClick, children, disabled }: { pressed: boolean; onClick: () => void; children: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className={clsx("h-9 rounded-md border px-3 text-[0.9375rem] hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50", pressed ? "border-accent bg-[color-mix(in_srgb,var(--accent)_22%,transparent)] text-fg" : "border-ctl text-fg")}
    >
      {children}
    </button>
  );
}

export function Toolbar({ fileId }: { fileId: string }) {
  const doc = useDocs((s) => s.docs[fileId]);
  if (!doc) return null;
  const frozen = doc.readOnly || doc.conflict || !doc.editable;
  const root = drillTarget(doc);
  return (
    <div role="toolbar" aria-label="Form tools" className="flex flex-wrap items-center gap-2 border-y border-line px-4 py-2.5">
      <div className="relative min-w-[200px] max-w-sm flex-1">
        <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted" aria-hidden />
        <input
          id={FIELD_SEARCH_ID}
          type="search"
          role="searchbox"
          aria-label="Search fields"
          placeholder="Search fields"
          value={doc.query}
          onChange={(e) => setQuery(fileId, e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && doc.query && (e.stopPropagation(), setQuery(fileId, ""))}
          className="h-9 w-full rounded-md border border-ctl bg-transparent pl-9 pr-8 text-[0.9375rem] placeholder:text-muted [&::-webkit-search-cancel-button]:hidden"
        />
        {doc.query ? (
          <button type="button" aria-label="Clear search" onClick={() => setQuery(fileId, "")} className="absolute right-1.5 top-1/2 grid size-6 -translate-y-1/2 place-items-center rounded text-muted hover:bg-hover hover:text-fg">
            <X className="size-3.5" aria-hidden />
          </button>
        ) : null}
      </div>
      <Toggle pressed={doc.changedOnly} onClick={() => setChangedOnly(fileId, !doc.changedOnly)}>Changed only</Toggle>
      <Toggle pressed={doc.showAdvanced} onClick={() => setShowAdvanced(fileId, !doc.showAdvanced)}>Show advanced</Toggle>
      <Toggle pressed={doc.selecting && !frozen} disabled={frozen} onClick={() => setSelecting(fileId, !doc.selecting)}>Select</Toggle>
      <div className="ml-auto flex items-center gap-2">
        {root && !frozen ? (
          <FieldEnvContext.Provider value={{ fileId, frozen, hideSecrets: true, showAdvanced: false, filterActive: false, visible: null }}>
            <AddSettingMenu parent={root} label="this view" />
          </FieldEnvContext.Provider>
        ) : null}
        <IconButton label="Undo" disabled={!doc.canUndo || frozen} onClick={() => void undo(fileId)}>
          <Undo2 className="size-4" aria-hidden />
        </IconButton>
        <IconButton label="Redo" disabled={!doc.canRedo || frozen} onClick={() => void redo(fileId)}>
          <Redo2 className="size-4" aria-hidden />
        </IconButton>
      </div>
    </div>
  );
}
