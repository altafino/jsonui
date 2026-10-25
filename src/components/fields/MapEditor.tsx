import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { addMember, removeAt, renameKey, useDocs } from "../../lib/store/doc-store";
import { Button } from "../ui/Button";
import { Field, ScalarControl } from "./Field";
import { FieldError, INPUT, TextControl } from "./controls";
import { idFor, jsonString, useEnv, useHint, useNode } from "./context";
import { ItemSelect, PROTECTED_REASON, rowRemoveClass } from "./Removal";

/** Editable key/value table for open maps (env vars etc.): add, rename, remove; duplicate keys rejected. */
export function MapEditor({ pointer, level }: { pointer: string; level: number }) {
  const { fileId, frozen } = useEnv();
  const node = useNode(pointer);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [err, setErr] = useState<string | null>(null);
  if (!node?.childPointers) return null;
  const keys = node.childPointers.map((p) => nodes?.[p]?.key ?? "");

  const add = async () => {
    const k = key.trim();
    if (!k) return setErr("Enter a key");
    if (keys.includes(k)) return setErr(`Key "${k}" already exists`);
    setErr(null);
    if (await addMember(fileId, pointer, k, jsonString(value))) {
      setKey("");
      setValue("");
    }
  };

  return (
    <div>
      {node.childPointers.length === 0 ? <p className="row-h flex items-center px-4 text-sm text-muted">No entries</p> : null}
      {node.childPointers.map((p) => (
        <MapRow key={p} pointer={p} level={level} siblings={keys} />
      ))}
      <div className="border-b border-line px-4 py-2">
        <div className="flex items-start gap-2">
          <input aria-label="New entry key" placeholder="name" value={key} disabled={frozen} onChange={(e) => (setKey(e.target.value), setErr(null))} onKeyDown={(e) => e.key === "Enter" && void add()} className={INPUT} spellCheck={false} />
          <input aria-label="New entry value" placeholder="value" value={value} disabled={frozen} onChange={(e) => setValue(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void add()} className={INPUT} spellCheck={false} />
          <Button onClick={() => void add()} disabled={frozen} className="shrink-0">
            <Plus className="size-4" aria-hidden /> Add
          </Button>
        </div>
        <FieldError id={idFor(pointer, "map-err")} message={err} />
      </div>
    </div>
  );
}

function MapRow({ pointer, level, siblings }: { pointer: string; level: number; siblings: string[] }) {
  const { fileId, frozen } = useEnv();
  const node = useNode(pointer);
  const hint = useHint(pointer);
  const [draft, setDraft] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  if (!node) return null;
  if (node.kind === "object" || node.kind === "array") return <Field pointer={pointer} level={level} />;
  const k = node.key ?? "";
  const labelId = idFor(pointer, "label");
  const commit = async () => {
    if (draft === null || draft === k) return setDraft(null);
    const next = draft.trim();
    if (!next) return setErr("Key cannot be empty");
    if (siblings.includes(next)) return setErr(`Key "${next}" already exists`);
    setErr(null);
    setDraft(null);
    await renameKey(fileId, pointer, next);
  };
  return (
    <div id={`row-${pointer}`} data-pointer={pointer} className="group border-b border-line px-4 py-1.5 hover:bg-hover">
      <div className="flex items-start gap-2">
        <div className="pt-px"><ItemSelect pointer={pointer} label={k} isProtected={hint?.protected} /></div>
        <div className="w-2/5 min-w-0">
          <input
            aria-label={`Key ${k}`}
            value={draft ?? k}
            disabled={frozen || !!hint?.protected}
            title={hint?.protected ? PROTECTED_REASON : undefined}
            spellCheck={false}
            aria-invalid={err ? true : undefined}
            onChange={(e) => (setDraft(e.target.value), setErr(null))}
            onBlur={() => void commit()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commit();
              if (e.key === "Escape") (setDraft(null), setErr(null));
            }}
            className={INPUT}
          />
          <FieldError id={idFor(pointer, "key-err")} message={err} />
        </div>
        <span id={labelId} className="sr-only">Value of {k}</span>
        <div className="min-w-0 flex-1">
          {node.kind === "string" ? <TextControl node={node} hint={hint} labelId={labelId} disabled={frozen} /> : <ScalarControl node={node} hint={hint} labelId={labelId} disabled={frozen} />}
        </div>
        {hint?.protected ? null : (
          <button type="button" disabled={frozen} aria-label={`Remove ${k}`} title={`Remove ${k}`} onClick={() => void removeAt(fileId, pointer)} className={`${rowRemoveClass} size-[var(--ctl-h)]`}>
            <Trash2 className="size-4" aria-hidden />
          </button>
        )}
      </div>
    </div>
  );
}
