import clsx from "clsx";
import { ChevronDown, ChevronRight, Copy, Ellipsis, GripVertical, Plus, X } from "lucide-react";
import { useState } from "react";
import { treeToJson } from "../../lib/json/tree";
import { summarizeItem } from "../../lib/json/summary";
import { childPointer } from "../../lib/json/pointer";
import { api } from "../../lib/tauri-api";
import { insertItem, moveItem, patch, removeAt, toggleExpanded, useDocs } from "../../lib/store/doc-store";
import { isContainer, type CachedNode } from "../../lib/store/cache";
import type { TreeNode } from "../../lib/types/generated/TreeNode";
import { Button } from "../ui/Button";
import { DropdownMenu } from "../ui/Menu";
import { ChangedDot } from "../ui/Chip";
import { Children } from "./Containers";
import { ScalarControl } from "./Field";
import { idFor, useEnv, useHint, useNode } from "./context";
import { ItemSelect } from "./Removal";

const PAGE = 100;

const asTree = (nodes: Record<string, CachedNode>, p: string): TreeNode => {
  const n = nodes[p];
  return { ...n, children: n.childPointers ? n.childPointers.map((c) => asTree(nodes, c)) : null };
};

/** Default value for a new item, matching what the list already holds. */
function newItemJson(nodes: Record<string, CachedNode>, ptrs: string[]): string {
  const last = ptrs.length ? nodes[ptrs[ptrs.length - 1]] : undefined;
  switch (last?.kind) {
    case "number": return "0";
    case "boolean": return "false";
    case "object": return "{}";
    case "array": return "[]";
    default: return '""';
  }
}

/** Ordered list editor: scalar items inline, container items as collapsible flat rows (DD17). */
export function ArrayList({ pointer, level }: { pointer: string; level: number }) {
  const { fileId, frozen } = useEnv();
  const node = useNode(pointer);
  const hint = useHint(pointer);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const [shown, setShown] = useState(PAGE);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  if (!node?.childPointers || !nodes) return null;
  const ptrs = node.childPointers;
  const kinds = new Set(ptrs.map((p) => nodes[p]?.kind));
  const mixed = kinds.size > 1;
  const add = () => void insertItem(fileId, pointer, ptrs.length, newItemJson(nodes, ptrs)).then(() => patch(fileId, { focusPointer: childPointer(pointer, ptrs.length) }));
  const move = (from: number, to: number) => {
    if (to < 0 || to >= ptrs.length || from === to) return;
    void moveItem(fileId, pointer, from, to).then(() => patch(fileId, { focusPointer: childPointer(pointer, to) }));
  };
  const label = hint?.title ?? "List";

  return (
    <div role="list" aria-label={label}>
      {ptrs.length === 0 ? <p className="row-h flex items-center px-4 text-sm text-muted">Empty list</p> : null}
      {ptrs.slice(0, shown).map((p, i) => {
        const item = nodes[p];
        if (!item) return null;
        const rowProps = {
          pointer: p, index: i, count: ptrs.length, arrayPointer: pointer, level, move, mixed,
          dragging: dragFrom === i, onDragStart: () => setDragFrom(i), onDragEnd: () => setDragFrom(null),
          onDropOn: () => { if (dragFrom !== null) move(dragFrom, i); setDragFrom(null); },
          arrayLabel: label, itemSummary: hint?.itemSummary ?? null,
        };
        return isContainer(item) ? <ContainerItem key={p} {...rowProps} /> : <ScalarItem key={p} {...rowProps} />;
      })}
      {ptrs.length > shown ? (
        <div className="px-4 py-2">
          <Button onClick={() => setShown((s) => s + PAGE)}>Show {Math.min(PAGE, ptrs.length - shown)} more of {ptrs.length - shown}</Button>
        </div>
      ) : null}
      <div className="px-4 py-2">
        <Button onClick={add} disabled={frozen} aria-label={`Add item to ${label}`}>
          <Plus className="size-4" aria-hidden /> Add item
        </Button>
      </div>
    </div>
  );
}

interface ItemProps {
  pointer: string;
  index: number;
  count: number;
  arrayPointer: string;
  level: number;
  move: (from: number, to: number) => void;
  mixed: boolean;
  dragging: boolean;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDropOn: () => void;
  arrayLabel: string;
  itemSummary: string | null;
}

function Handle({ p, index, count, frozen }: { p: string; index: number; count: number; frozen: boolean }) {
  return (
    <button
      type="button"
      data-handle={p}
      disabled={frozen}
      aria-label={`Reorder item ${index + 1} of ${count}. Alt+Up or Alt+Down moves it.`}
      className="grid size-8 shrink-0 cursor-grab place-items-center rounded-md text-muted hover:bg-hover hover:text-fg active:cursor-grabbing disabled:opacity-40"
    >
      <GripVertical className="size-4" aria-hidden />
    </button>
  );
}

const altMove = (e: React.KeyboardEvent, index: number, move: ItemProps["move"]) => {
  if (!e.altKey) return;
  if (e.key === "ArrowUp") (e.preventDefault(), move(index, index - 1));
  if (e.key === "ArrowDown") (e.preventDefault(), move(index, index + 1));
};

function ScalarItem(props: ItemProps) {
  const { fileId, frozen } = useEnv();
  const { pointer, index, count, move, arrayLabel } = props;
  const node = useNode(pointer);
  const hint = useHint(pointer);
  if (!node) return null;
  const labelId = idFor(pointer, "label");
  return (
    <div
      role="listitem"
      id={`row-${pointer}`}
      data-pointer={pointer}
      draggable={!frozen}
      onDragStart={props.onDragStart}
      onDragEnd={props.onDragEnd}
      onDragOver={(e) => e.preventDefault()}
      onDrop={props.onDropOn}
      onKeyDown={(e) => altMove(e, index, move)}
      className={clsx("group relative flex items-center gap-2 border-b border-line py-1 pl-3 pr-3 hover:bg-hover focus-within:bg-hover", props.dragging && "opacity-50")}
    >
      {node.changed ? <span className="absolute left-0.5 top-1/2 -translate-y-1/2"><ChangedDot /></span> : null}
      <ItemSelect pointer={pointer} label={`item ${index + 1}`} isProtected={hint?.protected} />
      <Handle p={pointer} index={index} count={count} frozen={frozen} />
      <span className="w-6 shrink-0 text-right mono text-sm text-muted" aria-hidden>{index + 1}</span>
      <span id={labelId} className="sr-only">{arrayLabel} item {index + 1}{props.mixed ? `, ${node.kind}` : ""}</span>
      <div className="min-w-0 flex-1 flex justify-end">
        <div className="w-full max-w-[26rem]">
          <ScalarControl node={node} hint={hint} labelId={labelId} disabled={frozen} />
        </div>
      </div>
      {props.mixed ? <span className="mono text-xs text-muted">{node.kind}</span> : null}
      {hint?.protected ? null : <button type="button" disabled={frozen} aria-label={`Remove item ${index + 1}`} title={`Remove item ${index + 1}`} onClick={() => void removeAt(fileId, pointer)} className="grid size-8 shrink-0 place-items-center rounded-md text-muted hover:bg-hover hover:text-fg disabled:opacity-50">
        <X className="size-4" aria-hidden />
      </button>}
    </div>
  );
}

function ContainerItem(props: ItemProps) {
  const { fileId, frozen, filterActive } = useEnv();
  const { pointer, index, count, move, arrayPointer, level, itemSummary } = props;
  const node = useNode(pointer);
  const hint = useHint(pointer);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const expanded = useDocs((s) => !!s.docs[fileId]?.expanded[pointer]);
  if (!node || !nodes) return null;
  const open = filterActive || expanded;
  const summary = node.kind === "object" ? summarizeItem(asTree(nodes, pointer), index, itemSummary) : `Item ${index + 1} · ${node.childCount ?? 0} ${node.childCount === 1 ? "item" : "items"}`;
  const bodyId = idFor(pointer, "body");
  const duplicate = async () => {
    const tree = await api.getSubtree(fileId, pointer, 64);
    await insertItem(fileId, arrayPointer, index + 1, treeToJson(tree));
  };
  return (
    <div role="listitem" id={`row-${pointer}`} data-pointer={pointer} data-testid="array-item" className={clsx(props.dragging && "opacity-50")}>
      <div
        draggable={!frozen}
        onDragStart={props.onDragStart}
        onDragEnd={props.onDragEnd}
        onDragOver={(e) => e.preventDefault()}
        onDrop={props.onDropOn}
        onKeyDown={(e) => altMove(e, index, move)}
        className="group relative flex items-center gap-2 border-b border-line row-h py-1 pl-3 pr-3 hover:bg-hover focus-within:bg-hover"
      >
        {node.changed ? <span className="absolute left-0.5 top-1/2 -translate-y-1/2"><ChangedDot /></span> : null}
        <ItemSelect pointer={pointer} label={`item ${index + 1}`} isProtected={hint?.protected} />
        <Handle p={pointer} index={index} count={count} frozen={frozen} />
        <button type="button" aria-expanded={open} aria-controls={bodyId} onClick={() => toggleExpanded(fileId, pointer)} className="flex min-w-0 flex-1 items-center gap-2 py-1 text-left">
          {open ? <ChevronDown className="size-4 shrink-0" aria-hidden /> : <ChevronRight className="size-4 shrink-0" aria-hidden />}
          <span className="w-6 shrink-0 text-right mono text-sm text-muted">{index + 1}</span>
          <span className="min-w-0 truncate">{summary}</span>
        </button>
        <DropdownMenu
          label={`Actions for item ${index + 1}`}
          trigger={
            <button type="button" aria-label={`Actions for item ${index + 1}`} className="grid size-8 shrink-0 place-items-center rounded-md text-muted hover:bg-hover hover:text-fg">
              <Ellipsis className="size-4" aria-hidden />
            </button>
          }
          entries={[
            { label: "Duplicate", icon: <Copy className="size-4" aria-hidden />, disabled: frozen, onSelect: () => void duplicate() },
            { label: "Move up", disabled: frozen || index === 0, onSelect: () => move(index, index - 1) },
            { label: "Move down", disabled: frozen || index === count - 1, onSelect: () => move(index, index + 1) },
            ...(hint?.protected ? [] : [{ label: "Remove", danger: true, disabled: frozen, separatorBefore: true, onSelect: () => void removeAt(fileId, pointer) }]),
          ]}
        />
      </div>
      {open ? (
        <div id={bodyId} className="cd-expand ml-8 border-l border-line">
          <Children parent={pointer} level={level + 1} />
        </div>
      ) : null}
    </div>
  );
}
