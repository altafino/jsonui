import { useState } from "react";
import { removeFile, renameFile, resolveUnsaved } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { confirmLinkAndSave, reloadDoc, useDocs } from "../../lib/store/doc-store";
import { toCoreError } from "../../lib/tauri-api";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

/** Switching documents with a draft: Save / Discard / Cancel (spec §10). */
export function UnsavedDialog({ fromId }: { fromId: string }) {
  const close = useApp((s) => s.closeDialog);
  const name = useApp((s) => s.files.find((f) => f.id === fromId)?.displayName ?? "this file");
  const n = useDocs((s) => s.docs[fromId]?.dirtyCount ?? 0);
  return (
    <Dialog
      open
      onClose={close}
      title="Unsaved changes"
      description={`${name} has ${n} unsaved ${n === 1 ? "change" : "changes"}.`}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="danger" onClick={() => void resolveUnsaved("discard")}>Discard</Button>
          <Button variant="primary" onClick={() => void resolveUnsaved("save")}>Save</Button>
        </>
      }
    >
      <p className="text-sm text-muted">Save writes them to the file after making a backup. Discard drops them.</p>
    </Dialog>
  );
}

/** One-time confirmation for symlink write-through or hard link detach (eng R5). */
export function LinkDialog({ fileId, code }: { fileId: string; code: "SYMLINK_REQUIRES_CONFIRMATION" | "HARDLINK_REQUIRES_CONFIRMATION" }) {
  const close = useApp((s) => s.closeDialog);
  const status = useApp((s) => s.statuses[fileId]);
  const sym = code === "SYMLINK_REQUIRES_CONFIRMATION";
  return (
    <Dialog
      open
      onClose={close}
      title={sym ? "Save through a symbolic link?" : "Save a file with other hard links?"}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" onClick={() => void confirmLinkAndSave(fileId)}>Confirm and save</Button>
        </>
      }
    >
      {sym ? (
        <p className="text-sm">
          This file is a link to <span className="mono">{status?.linkTarget ?? "another file"}</span>. Saving writes to that target. You will only be asked once for this file.
        </p>
      ) : (
        <p className="text-sm">This file has {Math.max(0, (status?.hardlinkCount ?? 2) - 1)} other hard {(status?.hardlinkCount ?? 2) === 2 ? "link" : "links"}. Saving replaces the file and detaches them. You will only be asked once for this file.</p>
      )}
    </Dialog>
  );
}

export function RenameDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const current = useApp((s) => s.files.find((f) => f.id === fileId)?.displayName ?? "");
  const [name, setName] = useState(current);
  const [err, setErr] = useState<string | null>(null);
  const submit = async () => {
    if (!name.trim()) return setErr("Enter a name");
    try {
      await renameFile(fileId, name.trim());
      close();
    } catch (e) {
      setErr(toCoreError(e).message);
    }
  };
  return (
    <Dialog
      open
      onClose={close}
      title="Rename display name"
      description="Only the name shown in ConfigDeck changes. The file on disk keeps its name."
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" onClick={() => void submit()}>Rename</Button>
        </>
      }
    >
      <label className="block text-sm" htmlFor="rename-input">Display name</label>
      <input id="rename-input" autoFocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void submit()} className="mt-1 h-9 w-full rounded-md border border-ctl bg-transparent px-3" />
      {err ? <p role="alert" className="mt-1 text-sm text-danger">{err}</p> : null}
    </Dialog>
  );
}

export function RemoveDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const file = useApp((s) => s.files.find((f) => f.id === fileId));
  const sample = file?.entryKind === "sample";
  return (
    <Dialog
      open
      onClose={close}
      title={`Remove ${file?.displayName ?? "file"}?`}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="danger" onClick={() => void removeFile(fileId).then(close)}>{sample ? "Remove and delete the sample copy" : "Remove from ConfigDeck"}</Button>
        </>
      }
    >
      <p className="text-sm">
        {sample ? "This deletes the sample copy in ConfigDeck's data folder." : "ConfigDeck stops managing this file. The file itself is not deleted."}
      </p>
    </Dialog>
  );
}

export function ReloadDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const n = useDocs((s) => s.docs[fileId]?.dirtyCount ?? 0);
  return (
    <Dialog
      open
      onClose={close}
      title="Discard changes and reload?"
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="danger" onClick={() => void reloadDoc(fileId).then(close)}>Discard &amp; reload</Button>
        </>
      }
    >
      <p className="text-sm">Reloading reads the file from disk and drops your {n} unsaved {n === 1 ? "change" : "changes"}.</p>
    </Dialog>
  );
}
