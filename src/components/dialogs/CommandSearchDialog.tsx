import * as RD from "@radix-ui/react-dialog";
import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { displayPath, humanize } from "../../lib/json/pointer";
import { useApp } from "../../lib/store/app-store";
import { ensureFullyLoaded, revealPointer, useDocs } from "../../lib/store/doc-store";
import { friendlyPath } from "../diff/ChangeList";

/** Ctrl+K: jump to a field of the open file (workspace-wide search is P1). Combobox over a listbox. */
export function CommandSearchDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const hints = useDocs((s) => s.docs[fileId]?.hints);
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => void ensureFullyLoaded(fileId), [fileId]);

  const results = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const all = Object.values(nodes ?? {}).filter((n) => n.pointer !== "");
    const scored = all
      .map((n) => {
        const label = hints?.[n.pointer]?.title ?? (n.key !== null ? humanize(n.key) : `Item ${(n.index ?? 0) + 1}`);
        const hay = `${label} ${n.key ?? ""} ${displayPath(n.pointer)} ${hints?.[n.pointer]?.description ?? ""}`.toLowerCase();
        return { n, label, hit: !needle || hay.includes(needle), depth: n.pointer.split("/").length };
      })
      .filter((x) => x.hit);
    return scored.sort((a, b) => a.depth - b.depth || a.n.pointer.localeCompare(b.n.pointer)).slice(0, 50);
  }, [nodes, hints, q]);

  useEffect(() => setActive(0), [q]);
  useEffect(() => {
    listRef.current?.children[active]?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const go = (i: number) => {
    const r = results[i];
    if (!r) return;
    close();
    void revealPointer(fileId, r.n.pointer);
  };

  return (
    <RD.Root open onOpenChange={(o) => !o && close()}>
      <RD.Portal>
        <RD.Overlay className="fixed inset-0 z-40" style={{ background: "var(--overlay)" }} />
        <RD.Content className="cd-popover fixed left-1/2 top-[14vh] z-50 w-[min(640px,90vw)] -translate-x-1/2 overflow-hidden rounded-lg border border-line bg-sidebar" data-testid="command-search">
          <RD.Title className="sr-only">Go to setting</RD.Title>
          <RD.Description className="sr-only">Type to find a field. Enter jumps to it and leaves the rest of the form visible.</RD.Description>
          <div className="flex items-center gap-2 border-b border-line px-4">
            <Search className="size-4 text-muted" aria-hidden />
            <input
              autoFocus
              role="combobox"
              aria-expanded
              aria-controls="cmd-list"
              aria-activedescendant={results[active] ? `cmd-opt-${active}` : undefined}
              aria-label="Go to setting"
              placeholder="Go to setting"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") (e.preventDefault(), setActive((a) => Math.min(results.length - 1, a + 1)));
                if (e.key === "ArrowUp") (e.preventDefault(), setActive((a) => Math.max(0, a - 1)));
                if (e.key === "Enter") (e.preventDefault(), go(active));
              }}
              className="h-12 w-full bg-transparent text-[1rem] outline-none placeholder:text-muted"
            />
          </div>
          <ul id="cmd-list" ref={listRef} role="listbox" aria-label="Fields" className="max-h-[50vh] overflow-auto py-1">
            {results.length === 0 ? <li className="px-4 py-6 text-sm text-muted">{`No fields match "${q.trim()}"`}</li> : null}
            {results.map((r, i) => (
              <li key={r.n.pointer} id={`cmd-opt-${i}`} role="option" aria-selected={i === active} onMouseMove={() => setActive(i)} onClick={() => go(i)} className={`flex cursor-pointer items-center gap-3 px-4 py-2 ${i === active ? "bg-hover" : ""}`}>
                <span className="min-w-0 flex-1 truncate">{r.label}</span>
                <span className="mono truncate text-[0.8125rem] text-muted">{friendlyPath(r.n.pointer) === r.label ? displayPath(r.n.pointer) : displayPath(r.n.pointer)}</span>
              </li>
            ))}
          </ul>
          <p className="border-t border-line px-4 py-2 text-[0.8125rem] text-muted">Enter jumps to the field. It does not hide the others.</p>
        </RD.Content>
      </RD.Portal>
    </RD.Root>
  );
}
