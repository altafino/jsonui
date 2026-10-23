import { Copy, Ellipsis, ExternalLink, FolderOpen, History, Pencil, RefreshCw, Trash2 } from "lucide-react";
import { copyText } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { countSeverity, reloadDoc, useDocs } from "../../lib/store/doc-store";
import { api } from "../../lib/tauri-api";
import type { ManagedDocument } from "../../lib/types/generated/ManagedDocument";
import type { Scope } from "../../lib/types/generated/Scope";
import { IconButton } from "../ui/Button";
import { Chip, StatusChip } from "../ui/Chip";
import { DropdownMenu } from "../ui/Menu";
import { Tip } from "../ui/Tooltip";

export const SCOPE_LABEL: Record<Scope, string> = { user: "User", "project-shared": "Shared", "project-local": "Local", managed: "Managed", generic: "File" };

/** Path with the file name kept visible: `~/projects/…/feature-flags.json` (DD12). */
export function MiddlePath({ path, className }: { path: string; className?: string }) {
  const i = path.lastIndexOf("/");
  const head = i >= 0 ? path.slice(0, i + 1) : "";
  const tail = i >= 0 ? path.slice(i + 1) : path;
  return (
    <span className={`flex min-w-0 mono ${className ?? ""}`} title={path}>
      <span className="min-w-0 truncate">{head}</span>
      <span className="shrink-0">{tail}</span>
    </span>
  );
}

/** Identity and file-level actions only; Save lives in the footer (DD2). */
export function DocumentHeader({ file }: { file: ManagedDocument }) {
  const doc = useDocs((s) => s.docs[file.id]);
  const openDialog = useApp((s) => s.openDialog);
  if (!doc) return null;
  const errors = countSeverity(doc, "error");
  const warnings = countSeverity(doc, "warning");
  const reload = () => (doc.dirtyCount > 0 ? openDialog({ kind: "reload", fileId: file.id }) : void reloadDoc(file.id));
  return (
    <div role="group" className="flex items-start gap-4 px-4 pt-4 pb-3" aria-label="File header">
      <div className="min-w-0 flex-1">
        <h1 className="truncate text-xl font-semibold leading-tight">{file.displayName}</h1>
        <div className="mt-1 flex items-center gap-2 text-sm text-muted">
          <MiddlePath path={file.absolutePath} />
          <Tip content="Copy path">
            <button type="button" aria-label="Copy path" onClick={() => void copyText(file.absolutePath, "Path")} className="grid size-6 shrink-0 place-items-center rounded hover:bg-hover hover:text-fg">
              <Copy className="size-3.5" aria-hidden />
            </button>
          </Tip>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <Chip>{doc.kind === "jsonc" ? "JSONC" : "JSON"}</Chip>
          {doc.presetInfo ? <Chip title={`Preset version ${doc.presetInfo.version}`}>{doc.presetInfo.displayName}</Chip> : null}
          {file.discoveredScope ? <Chip>{SCOPE_LABEL[file.discoveredScope]}</Chip> : null}
          {file.entryKind === "sample" ? <Chip>Sample</Chip> : null}
          {doc.readOnly ? <StatusChip tone="warn">Read-only</StatusChip> : null}
          {!doc.editable ? <StatusChip tone="error">Invalid</StatusChip> : errors > 0 ? <StatusChip tone="error">{errors} {errors === 1 ? "error" : "errors"}</StatusChip> : warnings > 0 ? <StatusChip tone="warn">{warnings} {warnings === 1 ? "warning" : "warnings"}</StatusChip> : <StatusChip tone="ok">Valid</StatusChip>}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <IconButton label="Reload from disk" onClick={reload}>
          <RefreshCw className="size-4" aria-hidden />
        </IconButton>
        <IconButton label="History" onClick={() => openDialog({ kind: "history", fileId: file.id })}>
          <History className="size-4" aria-hidden />
        </IconButton>
        <DropdownMenu
          label="More file actions"
          trigger={
            <IconButton label="More file actions">
              <Ellipsis className="size-4" aria-hidden />
            </IconButton>
          }
          entries={[
            { label: "Open in external editor", icon: <ExternalLink className="size-4" aria-hidden />, onSelect: () => void api.openInExternalEditor(file.id) },
            { label: "Reveal in file manager", icon: <FolderOpen className="size-4" aria-hidden />, onSelect: () => void api.revealManagedFile(file.id) },
            { label: "Rename display name", icon: <Pencil className="size-4" aria-hidden />, onSelect: () => openDialog({ kind: "rename", fileId: file.id }) },
            { label: "Remove from ConfigDeck", icon: <Trash2 className="size-4" aria-hidden />, danger: true, separatorBefore: true, onSelect: () => openDialog({ kind: "remove", fileId: file.id }) },
          ]}
        />
      </div>
    </div>
  );
}
