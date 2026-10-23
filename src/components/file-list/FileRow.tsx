import clsx from "clsx";
import { CircleAlert, Copy, ExternalLink, FileText, FileX, FolderOpen, Lock, Monitor, Pencil, Trash2, TriangleAlert } from "lucide-react";
import { copyText, requestSelect } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { useDocs } from "../../lib/store/doc-store";
import { api } from "../../lib/tauri-api";
import type { ManagedDocument } from "../../lib/types/generated/ManagedDocument";
import { SCOPE_LABEL } from "../document/DocumentHeader";
import { ChangedDot, Chip } from "../ui/Chip";
import { ContextMenu } from "../ui/Menu";
import { Tip } from "../ui/Tooltip";

type RowState = { label: string; icon: React.ReactNode } | null;

function stateOf(file: ManagedDocument, ctx: { exists: boolean | undefined; writable: boolean | undefined; invalid: boolean; conflict: boolean }): RowState {
  if (ctx.exists === false) return { label: "Missing", icon: <FileX className="size-4 text-warn" aria-hidden /> };
  if (ctx.invalid) return { label: "Invalid", icon: <CircleAlert className="size-4 text-danger" aria-hidden /> };
  if (ctx.conflict) return { label: "Changed on disk", icon: <TriangleAlert className="size-4 text-warn" aria-hidden /> };
  if (file.readOnly || ctx.writable === false) return { label: "Read-only", icon: <Lock className="size-4 text-muted" aria-hidden /> };
  return null;
}

export function FileRow({ file, rail }: { file: ManagedDocument; rail: boolean }) {
  const active = useApp((s) => s.activeFileId === file.id);
  const status = useApp((s) => s.statuses[file.id]);
  const openDialog = useApp((s) => s.openDialog);
  const doc = useDocs((s) => s.docs[file.id]);
  const dirty = (doc?.dirtyCount ?? 0) > 0;
  const state = stateOf(file, { exists: status?.exists, writable: status?.writable, invalid: status?.editable === false || (!!doc && doc.phase === "ready" && !doc.editable), conflict: !!doc?.conflict });
  const Icon = file.discoveredScope === "project-local" ? Monitor : FileText;
  const mono = !file.presetId || file.presetId === "feature-flags" ? "mono text-[0.9375rem]" : "";
  const entries = [
    { label: "Open", icon: <FileText className="size-4" aria-hidden />, onSelect: () => requestSelect(file.id) },
    { label: "Rename display name", icon: <Pencil className="size-4" aria-hidden />, onSelect: () => openDialog({ kind: "rename", fileId: file.id }) },
    { label: "Reveal in file manager", icon: <FolderOpen className="size-4" aria-hidden />, onSelect: () => void api.revealManagedFile(file.id) },
    { label: "Copy path", icon: <Copy className="size-4" aria-hidden />, onSelect: () => void copyText(file.absolutePath, "Path") },
    { label: "Open in external editor", icon: <ExternalLink className="size-4" aria-hidden />, onSelect: () => void api.openInExternalEditor(file.id) },
    { label: "Remove from ConfigDeck", icon: <Trash2 className="size-4" aria-hidden />, danger: true, separatorBefore: true, onSelect: () => openDialog({ kind: "remove", fileId: file.id }) },
  ];
  const name = file.displayName;
  const a11yName = [name, dirty ? "unsaved changes" : null, state?.label].filter(Boolean).join(", ");

  return (
    <ContextMenu entries={entries}>
      <li>
        <Tip content={rail ? <span>{name}{state ? ` · ${state.label}` : ""}</span> : <span className="mono text-xs">{file.absolutePath}</span>} side="right">
          <button
            type="button"
            aria-current={active ? "true" : undefined}
            aria-label={a11yName}
            data-file-id={file.id}
            onClick={() => requestSelect(file.id)}
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
              const all = [...(e.currentTarget.closest("nav")?.querySelectorAll<HTMLElement>("button[data-file-id]") ?? [])];
              const i = all.indexOf(e.currentTarget);
              all[i + (e.key === "ArrowDown" ? 1 : -1)]?.focus();
              e.preventDefault();
            }}
            className={clsx(
              "relative flex w-full items-center gap-2 text-left hover:bg-hover",
              rail ? "h-10 justify-center px-0" : "min-h-10 pl-5 pr-3",
              active && "bg-hover before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:bg-accent",
            )}
          >
            <span className="relative shrink-0">
              <Icon className="size-[1.125rem] text-muted" aria-hidden />
              {rail && (dirty || state) ? <span className="absolute -right-1.5 -top-1.5 grid place-items-center rounded-full bg-sidebar [&_svg]:size-3">{dirty ? <ChangedDot /> : state?.icon}</span> : null}
            </span>
            {!rail ? (
              <>
                <span className={clsx("min-w-0 flex-1 truncate", mono)}>{name}</span>
                {dirty ? <span title="Unsaved changes"><ChangedDot /></span> : null}
                {state ? <span className="flex items-center gap-1 text-xs text-muted" data-testid="row-state">{state.icon}<span className="sr-only">{state.label}</span><span aria-hidden>{state.label === "Invalid" || state.label === "Missing" ? state.label : null}</span></span> : null}
                {file.entryKind === "sample" ? <Chip>Sample</Chip> : file.discoveredScope ? <Chip>{SCOPE_LABEL[file.discoveredScope]}</Chip> : null}
              </>
            ) : null}
          </button>
        </Tip>
      </li>
    </ContextMenu>
  );
}
