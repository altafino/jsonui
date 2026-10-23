import * as Tabs from "@radix-ui/react-tabs";
import { CircleAlert, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useApp } from "../../lib/store/app-store";
import { refreshChanges, revertChange, saveDocument } from "../../lib/store/doc-store";
import { api, toCoreError } from "../../lib/tauri-api";
import type { ChangeSummary } from "../../lib/types/generated/ChangeSummary";
import type { Diagnostic } from "../../lib/types/generated/Diagnostic";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { ChangeList } from "./ChangeList";
import { TextDiff } from "./TextDiff";

export function DiagnosticBlock({ items, tone, title }: { items: Diagnostic[]; tone: "warn" | "error"; title: string }) {
  if (!items.length) return null;
  const Icon = tone === "warn" ? TriangleAlert : CircleAlert;
  return (
    <div role={tone === "error" ? "alert" : "group"} aria-label={title} className={`mt-4 rounded-md border px-3 py-2 ${tone === "warn" ? "border-warn/60" : "border-danger/60"}`} data-testid={`${tone}-block`}>
      <p className={`flex items-center gap-2 text-sm font-medium ${tone === "warn" ? "text-warn" : "text-danger"}`}>
        <Icon className="size-4" aria-hidden /> {title}
      </p>
      <ul className="mt-1 space-y-0.5 text-sm">
        {items.map((d, i) => (
          <li key={i}>{d.message}</li>
        ))}
      </ul>
    </div>
  );
}

/** Review changes (DD15): Changes tab with per-change Revert, Text diff tab, warnings above the buttons. */
export function ReviewDialog({ fileId }: { fileId: string }) {
  const close = useApp((s) => s.closeDialog);
  const [sum, setSum] = useState<ChangeSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    try {
      setSum(await api.previewDraft(fileId));
    } catch (e) {
      setError(toCoreError(e).message);
    }
  }, [fileId]);
  useEffect(() => void load(), [load]);

  const revert = async (opIndex: number) => {
    if (await revertChange(fileId, opIndex)) {
      await load();
      await refreshChanges(fileId);
      useApp.getState().announce("Change reverted");
    }
  };

  const warnings = sum?.warnings.length ?? 0;
  const hasErrors = (sum?.errors.length ?? 0) > 0;
  const none = !!sum && sum.changes.length === 0;
  const save = async () => {
    setSaving(true);
    close();
    await saveDocument(fileId, { acceptWarnings: warnings > 0 });
    setSaving(false);
  };

  return (
    <Dialog
      open
      onClose={close}
      title="Review changes"
      description="Check what will be written. A backup of the current file is made first."
      wide
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" disabled={!sum || none || hasErrors || saving} onClick={() => void save()}>
            {warnings > 0 ? `Save with ${warnings} ${warnings === 1 ? "warning" : "warnings"}` : "Save changes"}
          </Button>
        </>
      }
    >
      {error ? <p role="alert" className="text-danger">{error}</p> : null}
      {!sum && !error ? <p className="py-6 text-sm text-muted" aria-busy="true">&nbsp;</p> : null}
      {sum ? (
        <Tabs.Root defaultValue="changes">
          <Tabs.List aria-label="Review views" className="flex gap-1 border-b border-line">
            {[["changes", `Changes (${sum.changes.length})`], ["diff", "Text diff"]].map(([v, t]) => (
              <Tabs.Trigger key={v} value={v} className="-mb-px border-b-2 border-transparent px-3 py-2 text-[0.9375rem] text-muted hover:text-fg data-[state=active]:border-accent data-[state=active]:text-fg">
                {t}
              </Tabs.Trigger>
            ))}
          </Tabs.List>
          <Tabs.Content value="changes" className="pt-3">
            <ChangeList changes={sum.changes} onRevert={(i) => void revert(i)} empty="No unsaved changes" label="Changes" />
          </Tabs.Content>
          <Tabs.Content value="diff" className="pt-3">
            <TextDiff fileId={fileId} hunks={sum.hunks} />
          </Tabs.Content>
          <DiagnosticBlock items={sum.errors} tone="error" title="Fix these errors to save" />
          <DiagnosticBlock items={sum.warnings} tone="warn" title={`${warnings} ${warnings === 1 ? "warning" : "warnings"} from the preset. You can still save.`} />
        </Tabs.Root>
      ) : null}
    </Dialog>
  );
}
