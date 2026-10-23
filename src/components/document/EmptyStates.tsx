import { FilePlus, FolderSearch, FlaskConical, FileX } from "lucide-react";
import type { ReactNode } from "react";
import { addFile, createMissing, relocate, trySample } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { Button } from "../ui/Button";

/** Context line plus one obvious next step, in utility language (DD5). */
export function EmptyState({ title, children, action }: { title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-start gap-3 px-6 py-8" data-testid="empty-state">
      <p className="text-fg">{title}</p>
      {children ? <p className="text-sm text-muted">{children}</p> : null}
      {action}
    </div>
  );
}

export const WELCOME_COPY = "Edit JSON settings as forms. Every save makes a backup first.";

export function Welcome() {
  const openDialog = useApp((s) => s.openDialog);
  return (
    <div className="grid h-full place-items-center px-6" data-testid="welcome">
      <div className="max-w-md">
        <h1 className="text-[1.25rem] font-semibold">ConfigDeck</h1>
        <p className="mt-2 text-fg">{WELCOME_COPY}</p>
        <div className="mt-6 flex flex-col items-stretch gap-2">
          <Button variant="primary" className="h-10 justify-start" onClick={() => void addFile()}>
            <FilePlus className="size-4" aria-hidden /> Add JSON file
          </Button>
          <Button className="h-10 justify-start" onClick={() => openDialog({ kind: "discover" })}>
            <FolderSearch className="size-4" aria-hidden /> Discover Claude Code settings
          </Button>
          <Button className="h-10 justify-start" onClick={() => void trySample()}>
            <FlaskConical className="size-4" aria-hidden /> Try with a sample
          </Button>
        </div>
      </div>
    </div>
  );
}

export function MissingFile({ fileId, path }: { fileId: string; path: string }) {
  const openDialog = useApp((s) => s.openDialog);
  return (
    <div className="flex flex-col items-start gap-3 px-6 py-8" data-testid="missing-file">
      <div className="flex items-center gap-2 text-fg">
        <FileX className="size-5 text-warn" aria-hidden />
        <p>This file no longer exists at <span className="mono">{path}</span></p>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" onClick={() => void relocate(fileId)}>Locate file</Button>
        <Button onClick={() => void createMissing(fileId)}>Create empty file</Button>
        <Button variant="danger" onClick={() => openDialog({ kind: "remove", fileId })}>Remove from ConfigDeck</Button>
      </div>
    </div>
  );
}
