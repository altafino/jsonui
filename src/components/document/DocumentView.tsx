import { useApp } from "../../lib/store/app-store";
import { reloadDoc, useDocs } from "../../lib/store/doc-store";
import { api } from "../../lib/tauri-api";
import { Button } from "../ui/Button";
import { Banners } from "./Banners";
import { DocumentHeader, MiddlePath } from "./DocumentHeader";
import { MissingFile } from "./EmptyStates";
import { Form } from "./Form";
import { SourceViewPanel } from "./SourceViewPanel";
import { Toolbar } from "./Toolbar";

export function DocumentView({ fileId }: { fileId: string }) {
  const file = useApp((s) => s.files.find((f) => f.id === fileId));
  const doc = useDocs((s) => s.docs[fileId]);
  const openDialog = useApp((s) => s.openDialog);
  if (!file) return null;

  // No placeholder while a fast open resolves (spec §17).
  if (!doc || doc.phase === "loading") return <div className="h-full" aria-busy="true" />;

  if (doc.phase === "missing") {
    return (
      <div className="h-full overflow-auto">
        <div className="px-4 pt-4">
          <h1 className="truncate text-xl font-semibold">{file.displayName}</h1>
        </div>
        <MissingFile fileId={fileId} path={file.absolutePath} />
      </div>
    );
  }

  if (doc.phase === "error") {
    const e = doc.loadError;
    return (
      <div className="h-full overflow-auto">
        <div className="px-4 pt-4">
          <h1 className="truncate text-xl font-semibold">{file.displayName}</h1>
          <div className="mt-1 text-sm text-muted"><MiddlePath path={file.absolutePath} /></div>
        </div>
        <div role="alert" className="m-4 rounded-md border border-danger/60 p-4">
          <p className="font-medium">This file can&apos;t be opened.</p>
          <p className="mt-1 text-sm">{e?.message}</p>
          {e?.remediation ? <p className="mt-1 text-sm text-muted">{e.remediation}</p> : null}
          <p className="mono mt-2 text-[0.8125rem] text-muted">{e?.code}</p>
          <div className="mt-3 flex gap-2">
            <Button onClick={() => void api.openInExternalEditor(fileId)}>Open in editor</Button>
            <Button onClick={() => void reloadDoc(fileId)}>Reload</Button>
            <Button variant="danger" onClick={() => openDialog({ kind: "remove", fileId })}>Remove from ConfigDeck</Button>
          </div>
        </div>
      </div>
    );
  }

  const invalid = !doc.editable && doc.sourceView;
  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="document">
      <DocumentHeader file={file} />
      <Banners file={file} fileId={fileId} />
      {invalid ? (
        <div className="min-h-0 flex-1 overflow-auto pt-3">
          <SourceViewPanel fileId={fileId} view={doc.sourceView!} diagnostics={doc.syncDiagnostics} />
        </div>
      ) : (
        <>
          <div className="mt-3">
            <Toolbar fileId={fileId} />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden" data-testid="form-scroll">
            <Form fileId={fileId} />
          </div>
        </>
      )}
    </div>
  );
}
