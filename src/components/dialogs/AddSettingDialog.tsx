import { useState } from "react";
import { humanize } from "../../lib/json/pointer";
import { useApp } from "../../lib/store/app-store";
import { addMember, patch, useDocs } from "../../lib/store/doc-store";
import { childPointer } from "../../lib/json/pointer";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { TYPE_DEFAULTS } from "../fields/controls";

/** Add a property with a chosen JSON type. Schema-known names are prefilled; the value is never a silent default. */
export function AddSettingDialog({ fileId, parent, name }: { fileId: string; parent: string; name?: string }) {
  const close = useApp((s) => s.closeDialog);
  const siblings = useDocs((s) => {
    const d = s.docs[fileId];
    return (d?.nodes[parent]?.childPointers ?? []).map((p) => d.nodes[p]?.key ?? "");
  });
  const hint = useDocs((s) => s.docs[fileId]?.hints[childPointer(parent, name ?? "")]);
  const [key, setKey] = useState(name ?? "");
  const [type, setType] = useState(hint?.enumValues?.includes("true") ? "false" : (hint?.enumValues ? hint.enumValues[0] : '""'));
  const [err, setErr] = useState<string | null>(null);

  const options = hint?.enumValues ? hint.enumValues.map((v) => ({ label: v.replace(/^"|"$/g, ""), json: v })) : TYPE_DEFAULTS;
  const submit = async () => {
    const k = key.trim();
    if (!k) return setErr("Enter a key");
    if (siblings.includes(k)) return setErr(`"${k}" already exists here`);
    if (await addMember(fileId, parent, k, type)) {
      patch(fileId, (d) => ({ expanded: { ...d.expanded, [parent]: true }, focusPointer: childPointer(parent, k) }));
      close();
    }
  };

  return (
    <Dialog
      open
      onClose={close}
      title="Add setting"
      description="The new setting is added to the draft. Nothing is written until you save."
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" onClick={() => void submit()}>Add setting</Button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <label htmlFor="as-key" className="block text-sm">Key</label>
          <input id="as-key" autoFocus={!name} value={key} readOnly={!!name} onChange={(e) => (setKey(e.target.value), setErr(null))} onKeyDown={(e) => e.key === "Enter" && void submit()} spellCheck={false} aria-invalid={err ? true : undefined} className="mono mt-1 h-9 w-full rounded-md border border-ctl bg-transparent px-3 read-only:opacity-80" />
          {name ? <p className="mt-1 text-sm text-muted">{hint?.title ?? humanize(name)}</p> : null}
          {err ? <p role="alert" className="mt-1 text-sm text-danger">{err}</p> : null}
        </div>
        <div>
          <label htmlFor="as-type" className="block text-sm">{hint?.enumValues ? "Value" : "Type"}</label>
          <select id="as-type" value={type} onChange={(e) => setType(e.target.value)} className="mt-1 h-9 w-full rounded-md border border-ctl bg-transparent px-3">
            {options.map((o) => (
              <option key={o.json} value={o.json}>{o.label}</option>
            ))}
          </select>
        </div>
      </div>
    </Dialog>
  );
}
