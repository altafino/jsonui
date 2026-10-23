import clsx from "clsx";
import { ChevronDown, ChevronRight, Ellipsis, Plus } from "lucide-react";
import { childPointer } from "../../lib/json/pointer";
import { useApp } from "../../lib/store/app-store";
import { MAX_INLINE_LEVEL, drillInto, toggleExpanded, useDocs } from "../../lib/store/doc-store";
import { isContainer, type CachedNode } from "../../lib/store/cache";
import { ChangedDot } from "../ui/Chip";
import { Button } from "../ui/Button";
import { DropdownMenu } from "../ui/Menu";
import { ArrayList } from "./ArrayList";
import { AbsentBooleanRow, Field, labelFor } from "./Field";
import { idFor, shouldShowKey, useEnv, useHint, useNode, useChangeFor } from "./context";
import { RowDiagnostics, useRowEntries } from "./FieldRow";
import { ItemSelect, PROTECTED_REASON, RowRemoval } from "./Removal";
import { MapEditor } from "./MapEditor";
import { humanize } from "../../lib/json/pointer";

export const summaryOf = (n: Pick<CachedNode, "kind" | "childCount">) =>
  n.kind === "array" ? `${n.childCount ?? 0} ${n.childCount === 1 ? "item" : "items"}` : `${n.childCount ?? 0} ${n.childCount === 1 ? "key" : "keys"}`;

/** Child rows of a container, honouring filters. Absent optional booleans slot in before the first container. */
export function Children({ parent, level, only, absent = true }: { parent: string; level: number; only?: (p: string) => boolean; absent?: boolean }) {
  const { fileId, visible, filterActive } = useEnv();
  const node = useNode(parent);
  const hint = useHint(parent);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  if (!node?.childPointers) return <p className="px-4 py-2 text-sm text-muted" aria-busy="true">&nbsp;</p>;
  const ptrs = node.childPointers.filter((p) => (!visible || visible.has(p)) && (!only || only(p)));
  const firstContainer = ptrs.findIndex((p) => isContainer(nodes?.[p]));
  const absentRows =
    absent && !filterActive && node.kind === "object"
      ? (hint?.addableProperties ?? []).map((name) => <AbsentBooleanRow key={"absent:" + name} parent={parent} name={name} />)
      : [];
  const out = ptrs.map((p) => <Field key={p} pointer={p} level={level} />);
  const at = firstContainer === -1 ? out.length : firstContainer;
  out.splice(at, 0, ...absentRows);
  return <>{out}</>;
}

/** Row for containers nested deeper than the inline limit: summary plus "Open ›" (DD3). */
function OpenRow({ node, label, fileId }: { node: CachedNode; label: string; fileId: string }) {
  return (
    <div id={`row-${node.pointer}`} className="border-b border-line hover:bg-hover">
      <div className="row-h flex items-center gap-3 py-1.5 pl-4 pr-3">
        <span className="min-w-0 flex-1 truncate text-fg">{label}</span>
        <span className="mono text-sm text-muted">{node.kind === "array" ? `[${summaryOf(node)}]` : `{${summaryOf(node)}}`}</span>
        <Button onClick={() => drillInto(fileId, node.pointer)} aria-label={`Open ${label}`}>
          Open <ChevronRight className="size-4" aria-hidden />
        </Button>
      </div>
    </div>
  );
}

export function AddSettingMenu({ parent, label }: { parent: string; label: string }) {
  const { fileId, frozen } = useEnv();
  const hint = useHint(parent);
  const openDialog = useApp((s) => s.openDialog);
  const names = hint?.addableProperties ?? [];
  return (
    <DropdownMenu
      label={`Add setting to ${label}`}
      trigger={
        <Button disabled={frozen} aria-label={`Add setting to ${label}`} className="h-8">
          <Plus className="size-4" aria-hidden /> Add setting
        </Button>
      }
      entries={[
        ...names.map((n) => ({ label: humanize(n), onSelect: () => openDialog({ kind: "addSetting", fileId, parent, name: n }) })),
        { label: "Custom key…", separatorBefore: names.length > 0, onSelect: () => openDialog({ kind: "addSetting", fileId, parent }) },
      ]}
    />
  );
}

export function ContainerField({ pointer, level }: { pointer: string; level: number }) {
  const { fileId, showAdvanced, filterActive, frozen } = useEnv();
  const node = useNode(pointer);
  const hint = useHint(pointer);
  const expanded = useDocs((s) => !!s.docs[fileId]?.expanded[pointer]);
  const change = useChangeFor(pointer);
  const entries = useRowEntries(pointer, node?.key !== null, !!hint?.protected);
  if (!node) return null;
  const label = labelFor(node, hint);
  if (level > MAX_INLINE_LEVEL) return <OpenRow node={node} label={label} fileId={fileId} />;
  const open = filterActive || expanded;
  const bodyId = idFor(pointer, "body");
  const labelId = idFor(pointer, "label");
  const showKey = shouldShowKey(node.key, hint?.title, showAdvanced);
  const isMap = node.kind === "object" && !!hint?.openMap;
  void change;
  return (
    <div id={`row-${pointer}`} data-pointer={pointer} data-changed={node.changed || undefined} className="relative">
      <div className="group relative flex items-center gap-2 border-b border-line row-h py-1 pl-4 pr-3 hover:bg-hover focus-within:bg-hover">
        {node.changed ? <span className="absolute left-1.5 top-1/2 -translate-y-1/2"><ChangedDot /></span> : null}
        <ItemSelect pointer={pointer} label={label} removable={node.key !== null} isProtected={hint?.protected} />
        <button
          type="button"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => toggleExpanded(fileId, pointer)}
          className="flex min-w-0 flex-1 items-center gap-2 py-1 text-left"
        >
          {open ? <ChevronDown className="size-4 shrink-0" aria-hidden /> : <ChevronRight className="size-4 shrink-0" aria-hidden />}
          <span className="min-w-0">
            <span id={labelId} className="block truncate font-medium">{label}</span>
            {showKey || showAdvanced || hint?.protected ? (
              <span className="block mono text-[0.8125rem] text-muted">
                {[showKey || showAdvanced ? node.key : null, showAdvanced ? node.kind : null].filter(Boolean).join(" · ")}
                {hint?.protected ? <>{showKey || showAdvanced ? " · " : ""}<span title={PROTECTED_REASON}>Required</span></> : null}
              </span>
            ) : null}
          </span>
          {!open ? <span className="mono text-sm text-muted">{summaryOf(node)}</span> : null}
        </button>
        {open && node.kind === "object" && !isMap ? <AddSettingMenu parent={pointer} label={label} /> : null}
        <RowRemoval pointer={pointer} label={label} removable={node.key !== null} isProtected={hint?.protected} />
        <DropdownMenu
          label={`Actions for ${label}`}
          trigger={
            <button type="button" aria-label={`Actions for ${label}`} className={clsx("grid size-8 shrink-0 place-items-center rounded-md text-muted hover:bg-hover hover:text-fg focus-visible:opacity-100", "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 data-[state=open]:opacity-100")}>
              <Ellipsis className="size-4" aria-hidden />
            </button>
          }
          entries={entries.map((e) => (e.label === "Remove field" ? { ...e, disabled: e.disabled || frozen } : e))}
        />
      </div>
      <RowDiagnostics pointer={pointer} />
      {open ? (
        <div id={bodyId} role="group" aria-labelledby={labelId} className="cd-expand ml-5 border-l border-line">
          {node.kind === "array" ? (
            <ArrayList pointer={pointer} level={level + 1} />
          ) : isMap ? (
            <MapEditor pointer={pointer} level={level + 1} />
          ) : node.childCount === 0 ? (
            <EmptyObject pointer={pointer} label={label} />
          ) : (
            <Children parent={pointer} level={level + 1} />
          )}
        </div>
      ) : null}
    </div>
  );
}

function EmptyObject({ pointer, label }: { pointer: string; label: string }) {
  return (
    <div className="row-h flex items-center gap-3 border-b border-line px-4 py-1.5">
      <span className="text-sm text-muted">Empty object</span>
      <AddSettingMenu parent={pointer} label={label} />
    </div>
  );
}

export { childPointer };
