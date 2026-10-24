import clsx from "clsx";
import { Copy, Ellipsis, Trash2 } from "lucide-react";
import type { ReactNode } from "react";
import { displayPath } from "../../lib/json/pointer";
import { copyText } from "../../lib/store/actions";
import { removeAt } from "../../lib/store/doc-store";
import type { FieldHint } from "../../lib/types/generated/FieldHint";
import { ChangedDot } from "../ui/Chip";
import { ContextMenu, DropdownMenu, type MenuEntry } from "../ui/Menu";
import { Tip } from "../ui/Tooltip";
import { ItemSelect, PROTECTED_REASON, RowRemoval } from "./Removal";
import { idFor, shouldShowKey, useChangeFor, useDiagnosticsFor, useEnv } from "./context";
import { CircleAlert, TriangleAlert } from "lucide-react";

export interface RowShellProps {
  pointer: string;
  label: string;
  hint?: FieldHint;
  /** Object member key; null for array items. */
  rawKey: string | null;
  /** Only informative hints: `null`, `not in file`, `mixed array`, mismatches. */
  typeHint?: string | null;
  kind?: string;
  changed: boolean;
  removable: boolean;
  control: ReactNode;
  /** Shown instead of the label cell text when the control is a custom widget. */
  labelId: string;
  className?: string;
  /** Left cell content before the label (chevron etc.). */
  lead?: ReactNode;
  indentClass?: string;
}

/** Protected settings get no remove entry at all (the reason shows in the row's second line). */
export function useRowEntries(pointer: string, removable: boolean, isProtected = false): MenuEntry[] {
  const { fileId, frozen } = useEnv();
  return [
    { label: "Copy JSON path", icon: <Copy className="size-4" aria-hidden />, onSelect: () => void copyText(displayPath(pointer), "JSON path") },
    ...(removable && !isProtected ? [{ label: "Remove field", icon: <Trash2 className="size-4" aria-hidden />, danger: true, disabled: frozen, separatorBefore: true, onSelect: () => void removeAt(fileId, pointer) } satisfies MenuEntry] : []),
  ];
}

export function RowDiagnostics({ pointer }: { pointer: string }) {
  const diags = useDiagnosticsFor(pointer);
  if (!diags.length) return null;
  return (
    <ul className="px-3 pb-2 pt-0.5 space-y-0.5" aria-label="Problems with this field">
      {diags.map((d, i) => (
        <li key={i} className={clsx("flex items-center gap-1.5 text-xs", d.severity === "error" ? "text-danger" : d.severity === "warning" ? "text-warn" : "text-muted")}>
          {d.severity === "error" ? <CircleAlert className="size-3.5 shrink-0" aria-hidden /> : <TriangleAlert className="size-3.5 shrink-0" aria-hidden />}
          <span>{d.severity === "error" ? "Error: " : d.severity === "warning" ? "Warning: " : ""}{d.message}</span>
        </li>
      ))}
    </ul>
  );
}

/** One flat divided form row: label cell + control cell (DD1). */
export function FieldRow({ pointer, label, hint, rawKey, typeHint, kind, changed, removable, control, labelId, className, lead, indentClass }: RowShellProps) {
  const { showAdvanced } = useEnv();
  const change = useChangeFor(pointer);
  const entries = useRowEntries(pointer, removable, !!hint?.protected);
  const descId = idFor(pointer, "desc");
  const showKey = shouldShowKey(rawKey, hint?.title, showAdvanced);
  const was = changed && change && change.kind === "changed" ? (change.masked ? "••••" : change.oldText) : null;
  const sub: ReactNode[] = [];
  if (showKey) sub.push(<span key="k" className="mono">{rawKey}</span>);
  if (typeHint) sub.push(<span key="t" className="mono">{typeHint}</span>);
  if (hint?.protected) sub.push(<span key="r" title={PROTECTED_REASON}>Required</span>);
  if (showAdvanced) sub.push(<span key="p" className="mono">{displayPath(pointer)}{kind ? ` · ${kind}` : ""}</span>);
  if (was !== null) sub.push(<span key="w" className="mono">was {was}</span>);
  if (changed && !was && change?.kind === "added") sub.push(<span key="n" className="mono">new</span>);
  const desc = hint?.description;

  return (
    <ContextMenu entries={entries}>
      <div id={`row-${pointer}`} data-pointer={pointer} data-changed={changed || undefined} className={clsx("group relative border-b border-line hover:bg-hover focus-within:bg-hover", className)}>
        <div className="field-grid row-h py-1.5 pl-4 pr-3">
          <div className={clsx("min-w-0 flex items-center gap-2", indentClass)}>
            {changed ? <span className="absolute left-1.5 top-1/2 -translate-y-1/2"><ChangedDot /></span> : null}
            <ItemSelect pointer={pointer} label={label} removable={removable} isProtected={hint?.protected} />
            {lead}
            <div className="min-w-0 flex-1">
              <Tip content={<span className="mono text-xs">{displayPath(pointer)}</span>} side="bottom">
                <span id={labelId} className="line-clamp-2 break-words leading-snug text-fg">{label}</span>
              </Tip>
              <div id={descId} className={clsx("text-[0.8125rem] text-muted leading-snug", sub.length === 0 && !changed && !desc && "sr-only")}>
                {sub.map((s, i) => (
                  <span key={i}>{i > 0 ? " · " : ""}{s}</span>
                ))}
                {changed ? <span className="sr-only">{was !== null ? `changed, was ${was}` : "changed"}</span> : null}
                {desc ? <span className="block text-[0.8125rem]">{desc}</span> : null}
              </div>
            </div>
          </div>
          <div className="flex min-w-0 items-center justify-end gap-1.5">
            <div className="min-w-0 flex-1 flex justify-end">{control}</div>
            <RowRemoval pointer={pointer} label={label} removable={removable} isProtected={hint?.protected} />
            <DropdownMenu
              label={`Actions for ${label}`}
              trigger={
                <button type="button" aria-label={`Actions for ${label}`} className="grid size-8 shrink-0 place-items-center rounded-md text-muted opacity-0 hover:bg-hover hover:text-fg focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100 data-[state=open]:opacity-100">
                  <Ellipsis className="size-4" aria-hidden />
                </button>
              }
              entries={entries}
            />
          </div>
        </div>
        <RowDiagnostics pointer={pointer} />
      </div>
    </ContextMenu>
  );
}
