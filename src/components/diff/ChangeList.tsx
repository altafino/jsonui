import { ArrowRight, Undo2 } from "lucide-react";
import { displayPath, humanize, parsePointer } from "../../lib/json/pointer";
import type { SemanticChange } from "../../lib/types/generated/SemanticChange";
import { Button } from "../ui/Button";
import { Chip } from "../ui/Chip";
import { Tip } from "../ui/Tooltip";

export const friendlyPath = (pointer: string) => {
  const t = parsePointer(pointer);
  return t.length === 0 ? "Whole file" : t.map((x) => (/^\d+$/.test(x) ? `#${Number(x) + 1}` : humanize(x))).join(" › ");
};

const VERB: Record<SemanticChange["kind"], string> = { added: "Added", removed: "Removed", changed: "Changed", moved: "Moved", renamed: "Renamed" };

/** One line per change: friendly path, old → new, optional Revert and "both changed" marker (DD15, DD16). */
export function ChangeList({
  changes, onRevert, overlapping, empty, label,
}: {
  changes: SemanticChange[];
  onRevert?: (opIndex: number) => void;
  overlapping?: Set<string>;
  empty: string;
  label: string;
}) {
  if (changes.length === 0) return <p className="py-3 text-sm text-muted">{empty}</p>;
  return (
    <ul aria-label={label} className="divide-y divide-[var(--border)] rounded-md border border-line">
      {changes.map((c, i) => (
        <li key={`${c.pointer}-${i}`} className="flex items-center gap-3 px-3 py-2" data-testid="change-row">
          <div className="min-w-0 flex-1">
            <p className="truncate text-fg">
              <span className="text-muted">{VERB[c.kind]} </span>
              {friendlyPath(c.pointer)}
            </p>
            <p className="mono flex flex-wrap items-center gap-x-2 text-[0.8125rem] text-muted">
              <span title={c.pointer}>{displayPath(c.pointer)}</span>
              {c.kind !== "added" && c.oldText !== null ? <span className="text-fg">{c.oldText}</span> : null}
              {c.kind === "changed" || c.kind === "renamed" || c.kind === "moved" ? <ArrowRight className="size-3" aria-label="to" /> : null}
              {c.kind !== "removed" && c.newText !== null ? <span className="text-fg">{c.newText}</span> : null}
            </p>
          </div>
          {overlapping?.has(c.pointer) ? <Chip className="border-warn/60 text-warn">both changed</Chip> : null}
          {onRevert ? (
            <Tip content={c.revertible ? null : `Revert later changes under ${displayPath(c.pointer)} first`}>
              <span>
                <Button className="h-8" disabled={!c.revertible || c.opIndex === null} onClick={() => c.opIndex !== null && onRevert(c.opIndex)} aria-label={`Revert ${friendlyPath(c.pointer)}`}>
                  <Undo2 className="size-3.5" aria-hidden /> Revert
                </Button>
              </span>
            </Tip>
          ) : null}
        </li>
      ))}
    </ul>
  );
}
