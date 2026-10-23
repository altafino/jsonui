import { TriangleAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useApp } from "../../lib/store/app-store";
import { patch, reapply, reloadDoc } from "../../lib/store/doc-store";
import { api, toCoreError } from "../../lib/tauri-api";
import type { ConflictInfo } from "../../lib/types/generated/ConflictInfo";
import { ChangeList } from "../diff/ChangeList";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Tip } from "../ui/Tooltip";

/** Conflict dialog (DD16): two change lists, "both changed" marks, Reapply only without overlap. */
export function ConflictDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const [info, setInfo] = useState<ConflictInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copyNote, setCopyNote] = useState<string | null>(null);

  useEffect(() => {
    api.getConflict(fileId).then(setInfo, (e) => setError(toCoreError(e).message));
  }, [fileId]);

  const overlapping = useMemo(() => new Set(info?.overlapping ?? []), [info]);
  const reason = !info ? null : info.diskParseError ? "The version on disk is not valid JSON, so your changes can't be replayed onto it." : overlapping.size > 0 ? "Some of your changes touch settings that also changed on disk." : null;

  const doReapply = async () => {
    if (await reapply(fileId)) {
      close();
      useApp.getState().announce("Your changes were reapplied to the new version. Review them, then save.");
    }
  };
  const doReload = async () => {
    close();
    await reloadDoc(fileId, "Reloaded the version from disk. Your draft was discarded.");
    patch(fileId, { conflict: false });
  };
  const doCopy = async () => {
    try {
      const copy = await api.saveDraftAsCopy(fileId);
      if (copy) {
        await useApp.getState().refreshFiles();
        setCopyNote(`Saved your version as “${copy.displayName}”. It is listed in the sidebar.`);
      }
    } catch (e) {
      setError(toCoreError(e).message);
    }
  };

  return (
    <Dialog
      open
      onClose={close}
      title="This file changed outside ConfigDeck"
      description="Nothing was overwritten. Saving stays paused until you choose how to continue."
      wide
      footer={
        <>
          <Button onClick={close}>Close</Button>
          <Button onClick={() => void doCopy()}>Save my version as a copy</Button>
          <Button onClick={() => void doReload()}>Discard my draft &amp; reload</Button>
          <Tip content={reason}>
            <span>
              <Button variant="primary" disabled={!info?.canReapply} onClick={() => void doReapply()}>
                Reapply my changes to the new version
              </Button>
            </span>
          </Tip>
        </>
      }
    >
      {error ? <p role="alert" className="mb-3 text-danger">{error}</p> : null}
      {copyNote ? <p role="status" className="mb-3 text-sm text-ok">{copyNote}</p> : null}
      {!info && !error ? <p className="py-6 text-sm text-muted" aria-busy="true">&nbsp;</p> : null}
      {info ? (
        <div className="space-y-5">
          {info.diskParseError ? (
            <p role="alert" className="flex items-start gap-2 rounded-md border border-danger/60 px-3 py-2 text-sm">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-danger" aria-hidden />
              <span>The version on disk is not valid JSON: {info.diskParseError.message}</span>
            </p>
          ) : null}
          <section aria-labelledby="c-disk">
            <h3 id="c-disk" className="mb-2 text-sm font-medium">Changed on disk</h3>
            <ChangeList changes={info.diskChanges} overlapping={overlapping} empty={info.diskParseError ? "Can't compare: the disk version doesn't parse." : "No differences found on disk."} label="Changed on disk" />
          </section>
          <section aria-labelledby="c-draft">
            <h3 id="c-draft" className="mb-2 text-sm font-medium">Your unsaved changes</h3>
            <ChangeList changes={info.draftChanges} overlapping={overlapping} empty="You have no unsaved changes." label="Your unsaved changes" />
          </section>
          {reason ? <p className="text-sm text-muted">{reason}</p> : null}
        </div>
      ) : null}
    </Dialog>
  );
}
