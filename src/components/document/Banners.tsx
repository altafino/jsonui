import { Info, Lock, TriangleAlert, X } from "lucide-react";
import type { ReactNode } from "react";
import { api, toCoreError } from "../../lib/tauri-api";
import { useApp } from "../../lib/store/app-store";
import { patch, reloadDoc, useDocs } from "../../lib/store/doc-store";
import type { ManagedDocument } from "../../lib/types/generated/ManagedDocument";
import { Button } from "../ui/Button";

function Banner({ tone, icon, children, actions, role = "status", testId }: { tone: "warn" | "info" | "error"; icon: ReactNode; children: ReactNode; actions?: ReactNode; role?: "status" | "alert"; testId?: string }) {
  const border = tone === "warn" ? "border-warn/60" : tone === "error" ? "border-danger/60" : "border-line";
  return (
    <div role={role} data-testid={testId} className={`mx-4 mt-3 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-md border ${border} px-3 py-2 text-sm`}>
      <span className="shrink-0" aria-hidden>{icon}</span>
      <div className="min-w-0 flex-1">{children}</div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}

const LOCAL_NOTE = "Personal to you. Keep this file out of version control.";
const SHARED_NOTE = "Shared with your team through version control.";
export const R1_HINT = "Claude Code v2.1.211 and later may read the local file from the git repository root. If this folder is not the root, check which file Claude uses.";

export function Banners({ file, fileId }: { file: ManagedDocument; fileId: string }) {
  const doc = useDocs((s) => s.docs[fileId]);
  const openDialog = useApp((s) => s.openDialog);
  if (!doc) return null;
  return (
    <>
      {doc.conflict ? (
        <Banner
          tone="warn"
          role="alert"
          testId="conflict-banner"
          icon={<TriangleAlert className="size-4 text-warn" />}
          actions={
            <>
              <Button onClick={() => openDialog({ kind: "conflict", fileId })}>Compare</Button>
              <Button onClick={() => void reloadDoc(fileId)}>Discard my draft &amp; reload</Button>
              <Button onClick={() => void api.saveDraftAsCopy(fileId).then(() => useApp.getState().refreshFiles())}>Save a copy</Button>
            </>
          }
        >
          <strong className="font-medium">This file changed outside ConfigDeck.</strong> Saving is paused until you resolve it.
        </Banner>
      ) : null}
      {file.readOnly || doc.readOnly ? (
        <Banner tone="info" icon={<Lock className="size-4" />}>
          {file.discoveredScope === "managed" ? "Managed settings are read-only. ConfigDeck will not change this file." : "This file is read-only. Change its permissions outside ConfigDeck to edit it."}
        </Banner>
      ) : null}
      {file.discoveredScope === "project-local" ? (
        <Banner tone="info" icon={<Info className="size-4" />}>
          <p>{LOCAL_NOTE}</p>
          <p className="mt-0.5 text-muted">{R1_HINT}</p>
        </Banner>
      ) : null}
      {file.discoveredScope === "project-shared" ? <Banner tone="info" icon={<Info className="size-4" />}>{SHARED_NOTE}</Banner> : null}
      {file.entryKind === "sample" ? <Banner tone="info" icon={<Info className="size-4" />}>Safe to edit. This is a copy in ConfigDeck&apos;s data folder.</Banner> : null}
      {doc.leftoverTemp.length > 0 ? (
        <Banner
          tone="warn"
          icon={<TriangleAlert className="size-4 text-warn" />}
          testId="leftover-banner"
          actions={
            <Button
              onClick={() =>
                void api
                  .removeLeftoverTempFiles(fileId)
                  .then(() => patch(fileId, { leftoverTemp: [] }))
                  .catch((e) => patch(fileId, { opError: toCoreError(e) }))
              }
            >
              Remove leftover {doc.leftoverTemp.length === 1 ? "file" : "files"}
            </Button>
          }
        >
          An interrupted save left {doc.leftoverTemp.length === 1 ? "a temporary file" : "temporary files"} next to this file: <span className="mono">{doc.leftoverTemp.join(", ")}</span>
        </Banner>
      ) : null}
      {doc.externalNote ? (
        <Banner tone="info" icon={<Info className="size-4" />} actions={<button type="button" aria-label="Dismiss" className="rounded p-1 hover:bg-hover" onClick={() => patch(fileId, { externalNote: null })}><X className="size-4" /></button>}>
          {doc.externalNote}
        </Banner>
      ) : null}
      {doc.opError && doc.opError.code !== "EXTERNAL_CHANGE_CONFLICT" ? (
        <Banner tone="error" role="alert" icon={<TriangleAlert className="size-4 text-danger" />} actions={<button type="button" aria-label="Dismiss" className="rounded p-1 hover:bg-hover" onClick={() => patch(fileId, { opError: null })}><X className="size-4" /></button>}>
          {doc.opError.message}
          {doc.opError.remediation ? <span className="text-muted"> {doc.opError.remediation}</span> : null}
        </Banner>
      ) : null}
    </>
  );
}
