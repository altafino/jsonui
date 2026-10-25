import clsx from "clsx";
import { CircleAlert, Loader, Check } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useApp } from "../../lib/store/app-store";
import { changePointers } from "../../lib/store/change-targets";
import { dismissSaveError, refreshChanges, revealPointer, undoSave, useDocs } from "../../lib/store/doc-store";
import { formatClock } from "../../lib/store/save-machine";
import { requestSave, useSaveInfo } from "../../lib/store/selectors";
import { Button } from "../ui/Button";
import { Tip } from "../ui/Tooltip";

/** Footer-only Review + Save (DD2) with the save feedback states (DD6). Status text is an aria-live region. */
export function SaveBar() {
  const info = useSaveInfo();
  const openDialog = useApp((s) => s.openDialog);
  const doc = useDocs((s) => (info.fileId ? s.docs[info.fileId] : undefined));
  const save = doc?.save ?? { phase: "idle" as const };
  const { dirty, gate } = info;
  const quiet = dirty === 0 && save.phase !== "error" && save.phase !== "saved" && save.phase !== "saving";

  let status: ReactNode;
  if (!doc) status = <span className="text-muted">Ready</span>;
  else if (save.phase === "saving") {
    status = (
      <span className="inline-flex items-center gap-2">
        {save.spinner ? <Loader className="size-4 animate-spin motion-reduce:animate-none" aria-hidden /> : null}
        {save.spinner ? "Saving…" : <span className="text-muted">{dirty} pending {dirty === 1 ? "change" : "changes"}</span>}
      </span>
    );
  } else if (save.phase === "saved" && dirty === 0) {
    status = (
      <span className="inline-flex flex-wrap items-center gap-x-2">
        <Check className="size-4 text-ok" aria-hidden />
        <span>{save.restored ? `Restored ${formatClock(save.at)}` : `Saved ${formatClock(save.at)}`}{save.backup ? " · backup created" : ""}</span>
        {save.backupId ? (
          <button type="button" onClick={() => void undoSave(info.fileId!)} className="text-accent-fg underline underline-offset-2 hover:no-underline">
            Undo save
          </button>
        ) : null}
      </span>
    );
  } else if (dirty > 0 && info.fileId) {
    const extra = (
      <>
        {!gate.enabled && gate.reason ? <span className="text-warn"> · {gate.reason}</span> : null}
        {info.warnings > 0 && gate.enabled ? <span className="text-warn"> · {info.warnings} {info.warnings === 1 ? "warning" : "warnings"}</span> : null}
      </>
    );
    status = <PendingChanges fileId={info.fileId} dirty={dirty} extra={extra} />;
  } else {
    status = <span className="text-muted">{save.phase === "settled" ? "All changes saved" : "No unsaved changes"}</span>;
  }

  return (
    <footer data-landmark="status" tabIndex={-1} aria-label="Save bar" className="border-t border-line bg-bg">
      {save.phase === "error" ? (
        <div role="alert" data-testid="save-error" className="flex flex-wrap items-center gap-3 border-b border-danger/50 px-4 py-2 text-sm">
          <CircleAlert className="size-4 shrink-0 text-danger" aria-hidden />
          <p className="min-w-0 flex-1">
            <span className="font-medium">Couldn&apos;t save. </span>
            {save.error.message}
            {save.error.remediation ? <span className="text-muted"> {save.error.remediation}</span> : null}
            <span className="mono ml-2 text-[0.8125rem] text-muted">{save.error.code}</span>
          </p>
          <Button variant="ghost" className="h-7" onClick={() => dismissSaveError(info.fileId!)}>Dismiss</Button>
        </div>
      ) : null}
      <div data-testid="save-bar" data-savebar={quiet ? "quiet" : "open"} className={clsx("flex items-center gap-3 px-4", quiet ? "h-10" : "h-[72px]")}>
        <div role="status" aria-live="polite" data-testid="save-status" className="min-w-0 flex-1 text-fg">{status}</div>
        {dirty > 0 ? (
          <>
            <Button onClick={() => info.fileId && openDialog({ kind: "review", fileId: info.fileId })}>Review changes</Button>
            <Tip content={gate.enabled ? null : gate.reason}>
              <span>
                <Button variant="primary" disabled={!gate.enabled} aria-disabled={!gate.enabled} onClick={requestSave} title={gate.reason ?? undefined} className="min-w-36 h-10">
                  Save changes
                </Button>
              </span>
            </Tip>
          </>
        ) : null}
      </div>
    </footer>
  );
}

/** "N pending changes — show", plus previous and next when more than one field changed. */
function PendingChanges({ fileId, dirty, extra }: { fileId: string; dirty: number; extra: ReactNode }) {
  const changes = useDocs((s) => s.docs[fileId]?.changes);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const ptrs = changePointers(changes ?? [], nodes ?? {});
  const [index, setIndex] = useState(0);
  const word = dirty === 1 ? "change" : "changes";

  const go = async (i: number) => {
    await refreshChanges(fileId);
    const d = useDocs.getState().docs[fileId];
    if (!d) return;
    const list = changePointers(d.changes, d.nodes);
    if (!list.length) return;
    const next = ((i % list.length) + list.length) % list.length;
    setIndex(next);
    await revealPointer(fileId, list[next]);
  };

  return (
    <span className="inline-flex min-w-0 flex-wrap items-center gap-x-2">
      <span className="min-w-0">
        Ready · {dirty} pending {word}
        {extra}
        {" — "}
      </span>
      <button type="button" aria-label={`Show pending ${word}`} onClick={() => void go(ptrs.length ? index : 0)} className="text-accent-fg underline underline-offset-2 hover:no-underline">
        show
      </button>
      {ptrs.length > 1 ? (
        <>
          <button type="button" aria-label="Previous change" onClick={() => void go(index - 1)} className="text-accent-fg underline underline-offset-2 hover:no-underline">
            Previous
          </button>
          <button type="button" aria-label="Next change" onClick={() => void go(index + 1)} className="text-accent-fg underline underline-offset-2 hover:no-underline">
            Next
          </button>
        </>
      ) : null}
    </span>
  );
}
