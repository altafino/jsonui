// Leaf controls. Each takes the cached node and sends ops through the doc store.
import * as RG from "@radix-ui/react-radio-group";
import * as Switch from "@radix-ui/react-switch";
import clsx from "clsx";
import { CircleAlert, Eye, EyeOff, Minus, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { checkNumberText, isIntegerText, stepInteger } from "../../lib/json/numbers";
import { addMember, patch, removeAt, setValue } from "../../lib/store/doc-store";
import type { CachedNode } from "../../lib/store/cache";
import type { FieldHint } from "../../lib/types/generated/FieldHint";
import { DropdownMenu } from "../ui/Menu";
import { Button } from "../ui/Button";
import { jsonString, useEnv } from "./context";

export const INPUT =
  "h-[var(--ctl-h)] w-full min-w-0 rounded-md border border-ctl bg-transparent px-3 mono text-[0.9375rem] text-fg placeholder:text-muted disabled:opacity-60 aria-[invalid=true]:border-danger";

export function FieldError({ id, message }: { id: string; message: string | null }) {
  if (!message) return null;
  return (
    <p id={id} role="alert" className="mt-1 flex items-center gap-1 text-xs text-danger">
      <CircleAlert className="size-3 shrink-0" aria-hidden />
      {message}
    </p>
  );
}

// ------------------------------------------------------------------ boolean

export function BooleanSwitch({ node, labelId, disabled }: { node: CachedNode; labelId: string; disabled?: boolean }) {
  const { fileId } = useEnv();
  const on = node.text === "true";
  return (
    <Switch.Root
      checked={on}
      disabled={disabled}
      aria-labelledby={labelId}
      onCheckedChange={(v) => void setValue(fileId, node.pointer, String(v))}
      className="relative h-6 w-11 shrink-0 rounded-full border border-ctl bg-transparent transition-colors data-[state=checked]:border-accent data-[state=checked]:bg-accent disabled:opacity-60"
    >
      <Switch.Thumb className="block size-4 translate-x-[3px] rounded-full bg-ctl transition-transform data-[state=checked]:translate-x-[22px] data-[state=checked]:bg-white" />
    </Switch.Root>
  );
}

export type Tri = "unset" | "on" | "off";

/** Optional boolean: Not set / On / Off radiogroup (spec §5, DD14). Not set means the key is absent. */
export function TriStateBoolean({
  pointer, parent, keyName, value, labelId, describedBy, disabled,
}: {
  pointer: string;
  parent: string;
  keyName: string;
  value: Tri;
  labelId: string;
  describedBy?: string;
  disabled?: boolean;
}) {
  const { fileId } = useEnv();
  const change = (next: string) => {
    const v = next as Tri;
    if (v === value) return;
    const done =
      v === "unset" ? removeAt(fileId, pointer)
      : value === "unset" ? addMember(fileId, parent, keyName, v === "on" ? "true" : "false")
      : setValue(fileId, pointer, v === "on" ? "true" : "false");
    // The row is re-created when the key appears or disappears; keep keyboard focus on the control.
    void done.then(() => patch(fileId, { focusPointer: pointer }));
  };
  const items: [Tri, string][] = [["unset", "Not set"], ["on", "On"], ["off", "Off"]];
  return (
    <RG.Root
      value={value}
      onValueChange={change}
      disabled={disabled}
      aria-labelledby={labelId}
      aria-describedby={describedBy}
      orientation="horizontal"
      className="inline-flex h-[var(--ctl-h)] w-full max-w-[17rem] overflow-hidden rounded-md border border-ctl"
    >
      {items.map(([v, text], i) => (
        <RG.Item
          key={v}
          value={v}
          className={clsx(
            "flex-1 px-3 text-sm text-fg hover:bg-hover disabled:opacity-60 data-[state=checked]:bg-[color-mix(in_srgb,var(--accent)_30%,transparent)] data-[state=checked]:font-semibold",
            i > 0 && "border-l border-ctl",
          )}
        >
          {text}
        </RG.Item>
      ))}
    </RG.Root>
  );
}

// ------------------------------------------------------------------ text

function useDraftText(value: string, commit: (v: string) => void) {
  const [text, setText] = useState(value);
  const dirty = useRef(false);
  useEffect(() => {
    setText(value);
    dirty.current = false;
  }, [value]);
  return {
    text,
    onChange: (v: string) => {
      dirty.current = true;
      setText(v);
    },
    finish: () => {
      if (dirty.current && text !== value) commit(text);
      dirty.current = false;
    },
    revert: () => {
      dirty.current = false;
      setText(value);
    },
  };
}

const looksSecret = (key: string, hint?: FieldHint) => !!hint?.secret || /(token|secret|password|passwd|api[-_]?key)/i.test(key);
const isUrl = (s: string) => {
  try {
    new URL(s);
    return true;
  } catch {
    return false;
  }
};

export function TextControl({ node, hint, labelId, descId, disabled }: { node: CachedNode; hint?: FieldHint; labelId: string; descId?: string; disabled?: boolean }) {
  const { fileId, hideSecrets } = useEnv();
  const value = node.text ?? "";
  const d = useDraftText(value, (v) => void setValue(fileId, node.pointer, jsonString(v)));
  const secret = looksSecret(node.key ?? "", hint) || hint?.format === "password";
  const [reveal, setReveal] = useState(false);
  const masked = secret && hideSecrets && !reveal;
  const multiline = !masked && (value.includes("\n") || d.text.includes("\n") || value.length > 80);
  const err =
    hint?.minLength != null && d.text.length < hint.minLength ? `Enter at least ${hint.minLength} characters`
    : hint?.maxLength != null && d.text.length > hint.maxLength ? `At most ${hint.maxLength} characters`
    : hint?.format === "uri" && d.text !== "" && !isUrl(d.text) ? "Not a valid URL"
    : null;
  const errId = `${labelId}-err`;
  const common = {
    "aria-labelledby": labelId,
    "aria-describedby": [descId, err ? errId : null].filter(Boolean).join(" ") || undefined,
    "aria-invalid": err ? true : undefined,
    disabled,
    value: d.text,
    onBlur: d.finish,
  };
  return (
    <div className="w-full min-w-0">
      <div className="flex items-start gap-1.5">
        {multiline ? (
          <textarea
            {...common}
            rows={Math.min(8, Math.max(3, d.text.split("\n").length))}
            onChange={(e) => d.onChange(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && d.revert()}
            className={clsx(INPUT, "h-auto py-2 resize-y")}
          />
        ) : (
          <input
            {...common}
            type={masked ? "password" : hint?.format === "uri" ? "url" : "text"}
            autoComplete="off"
            spellCheck={false}
            data-1p-ignore
            onChange={(e) => d.onChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") d.finish();
              if (e.key === "Escape") d.revert();
            }}
            className={INPUT}
          />
        )}
        {secret ? (
          <button
            type="button"
            aria-pressed={reveal}
            aria-label={reveal ? "Hide value" : "Reveal value"}
            title={reveal ? "Hide value" : "Reveal value"}
            onClick={() => setReveal((r) => !r)}
            className="grid size-[var(--ctl-h)] shrink-0 place-items-center rounded-md border border-ctl text-fg hover:bg-hover"
          >
            {reveal ? <EyeOff className="size-4" aria-hidden /> : <Eye className="size-4" aria-hidden />}
          </button>
        ) : null}
      </div>
      {hint?.maxLength != null ? <p className="mt-1 text-right text-xs text-muted mono">{d.text.length}/{hint.maxLength}</p> : null}
      <FieldError id={errId} message={err} />
    </div>
  );
}

// ------------------------------------------------------------------ number

export function NumberControl({ node, hint, labelId, descId, disabled }: { node: CachedNode; hint?: FieldHint; labelId: string; descId?: string; disabled?: boolean }) {
  const { fileId } = useEnv();
  const value = node.text ?? "";
  const rules = { integer: hint?.format === "integer", minimum: hint?.minimum, maximum: hint?.maximum };
  const check = (t: string) => checkNumberText(t, rules);
  const d = useDraftText(value, (v) => {
    if (check(v).ok) void setValue(fileId, node.pointer, v.trim());
  });
  const res = check(d.text);
  const err = res.ok ? null : res.reason;
  const errId = `${labelId}-err`;
  const stepper = isIntegerText(value);
  const step = (delta: 1 | -1) => {
    const next = stepInteger(d.text, delta);
    if (next && check(next).ok) void setValue(fileId, node.pointer, next);
  };
  return (
    <div className="w-full min-w-0">
      <div className="flex">
        <input
          type="text"
          inputMode="decimal"
          aria-labelledby={labelId}
          aria-describedby={[descId, err ? errId : null].filter(Boolean).join(" ") || undefined}
          aria-invalid={err ? true : undefined}
          disabled={disabled}
          value={d.text}
          spellCheck={false}
          onChange={(e) => d.onChange(e.target.value)}
          onBlur={() => (res.ok ? d.finish() : undefined)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && res.ok) d.finish();
            if (e.key === "Escape") d.revert();
            if (e.key === "ArrowUp" && stepper) (e.preventDefault(), step(1));
            if (e.key === "ArrowDown" && stepper) (e.preventDefault(), step(-1));
          }}
          className={clsx(INPUT, stepper && "rounded-r-none")}
        />
        {stepper ? (
          <div className="flex">
            {([-1, 1] as const).map((delta) => (
              <button
                key={delta}
                type="button"
                tabIndex={-1}
                disabled={disabled}
                aria-label={delta === 1 ? "Increase" : "Decrease"}
                onClick={() => step(delta)}
                className={clsx("grid w-9 place-items-center border border-ctl text-fg hover:bg-hover disabled:opacity-60", delta === 1 ? "rounded-r-md border-l-0" : "border-l-0")}
              >
                {delta === 1 ? <Plus className="size-4" aria-hidden /> : <Minus className="size-4" aria-hidden />}
              </button>
            ))}
          </div>
        ) : null}
      </div>
      <FieldError id={errId} message={err} />
    </div>
  );
}

// ------------------------------------------------------------------ enum

const parseEnum = (j: string): string => {
  try {
    const v = JSON.parse(j);
    return typeof v === "string" ? v : j;
  } catch {
    return j;
  }
};

/** Native select: keyboard and screen-reader behaviour for free. */
export function EnumSelect({ node, hint, labelId, descId, disabled }: { node: CachedNode; hint: FieldHint; labelId: string; descId?: string; disabled?: boolean }) {
  const { fileId } = useEnv();
  const options = hint.enumValues ?? [];
  const current = node.kind === "string" ? jsonString(node.text ?? "") : (node.text ?? "");
  const known = options.includes(current);
  return (
    <div className="relative w-full min-w-0">
      <select
        aria-labelledby={labelId}
        aria-describedby={descId}
        disabled={disabled}
        value={current}
        onChange={(e) => void setValue(fileId, node.pointer, e.target.value)}
        className={clsx(INPUT, "appearance-none pr-9")}
      >
        {!known ? <option value={current}>{parseEnum(current)} (not in list)</option> : null}
        {options.map((o) => (
          <option key={o} value={o}>{parseEnum(o)}</option>
        ))}
      </select>
      <svg aria-hidden viewBox="0 0 16 16" className="pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-fg"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </div>
  );
}

// ------------------------------------------------------------------ null

export const TYPE_DEFAULTS: { label: string; json: string }[] = [
  { label: "Text", json: '""' },
  { label: "Number", json: "0" },
  { label: "Yes / no", json: "false" },
  { label: "Object", json: "{}" },
  { label: "List", json: "[]" },
];

export function NullControl({ node, disabled }: { node: CachedNode; disabled?: boolean }) {
  const { fileId } = useEnv();
  return (
    <DropdownMenu
      label="Set value"
      trigger={<Button disabled={disabled}>Set value</Button>}
      entries={TYPE_DEFAULTS.map((t) => ({ label: `Set as ${t.label.toLowerCase()}`, onSelect: () => void setValue(fileId, node.pointer, t.json) }))}
    />
  );
}
