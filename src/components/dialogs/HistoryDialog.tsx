import { useEffect, useState } from "react";
import { useApp } from "../../lib/store/app-store";
import { restoreFromHistory, useDocs } from "../../lib/store/doc-store";
import { api, toCoreError } from "../../lib/tauri-api";
import type { BackupEntry } from "../../lib/types/generated/BackupEntry";
import type { ChangeSummary } from "../../lib/types/generated/ChangeSummary";
import { ChangeList } from "../diff/ChangeList";
import { TextDiff } from "../diff/TextDiff";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

const fmt = (iso: string) => new Date(iso).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
const size = (n: bigint | number) => (Number(n) < 1024 ? `${n} B` : `${(Number(n) / 1024).toFixed(1)} KB`);

/** History: backups list, preview what a restore would change, then restore through the normal safety checks. */
export function HistoryDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const dirty = useDocs((s) => s.docs[fileId]?.dirtyCount ?? 0);
  const [list, setList] = useState<BackupEntry[] | null>(null);
  const [sel, setSel] = useState<BackupEntry | null>(null);
  const [preview, setPreview] = useState<ChangeSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listBackups(fileId).then(setList, (e) => setError(toCoreError(e).message));
  }, [fileId]);

  const pick = async (b: BackupEntry) => {
    setSel(b);
    setPreview(null);
    try {
      setPreview(await api.previewRestore(fileId, b.id));
    } catch (e) {
      setError(toCoreError(e).message);
    }
  };

  return (
    <Dialog
      open
      onClose={close}
      title="History"
      description="A backup is made before every save. Restoring goes through the same safety checks as saving."
      wide
      footer={
        <>
          <Button onClick={close}>Close</Button>
          <Button variant="primary" disabled={!sel || dirty > 0} title={dirty > 0 ? "Save or discard your unsaved changes first" : undefined} onClick={() => sel && void restoreFromHistory(fileId, sel.id).then((ok) => ok && close())}>
            Restore this backup
          </Button>
        </>
      }
    >
      {error ? <p role="alert" className="mb-3 text-danger">{error}</p> : null}
      {list && list.length === 0 ? <p className="py-6 text-fg">No backups yet. One is created every time you save.</p> : null}
      {list && list.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
          <ul aria-label="Backups" className="divide-y divide-[var(--border)] rounded-md border border-line">
            {list.map((b) => (
              <li key={b.id}>
                <button type="button" aria-pressed={sel?.id === b.id} onClick={() => void pick(b)} className={`flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-hover ${sel?.id === b.id ? "bg-hover" : ""}`}>
                  <span>{fmt(b.createdAt)}</span>
                  <span className="mono text-[0.8125rem] text-muted">
                    {size(b.sizeBytes)}
                    {b.changeCount !== null ? ` · ${b.changeCount} ${b.changeCount === 1 ? "change" : "changes"}` : ""}
                  </span>
                  <span className="mono truncate text-[0.75rem] text-muted" title={b.location}>{b.location}</span>
                </button>
              </li>
            ))}
          </ul>
          <div aria-live="polite">
            {sel && !preview ? <p className="text-sm text-muted">Loading preview…</p> : null}
            {!sel ? <p className="text-sm text-muted">Select a backup to preview what restoring it would change.</p> : null}
            {preview ? (
              <div className="space-y-3">
                <h3 className="text-sm font-medium">Restoring would change the current file like this</h3>
                <ChangeList changes={preview.changes} empty="Identical to the current file." label="Restore preview" />
                {preview.hunks.length ? <TextDiff fileId={fileId} hunks={preview.hunks} /> : null}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </Dialog>
  );
}
