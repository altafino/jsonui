import { humanize, parentPointer } from "../../lib/json/pointer";
import type { CachedNode } from "../../lib/store/cache";
import type { FieldHint } from "../../lib/types/generated/FieldHint";
import { BooleanSwitch, EnumSelect, NullControl, NumberControl, TextControl, TriStateBoolean } from "./controls";
import { ContainerField } from "./Containers";
import { idFor, lastToken, useEnv, useHint, useNode } from "./context";
import { FieldRow } from "./FieldRow";

const isBoolEnum = (h?: FieldHint) => !!h?.enumValues && h.enumValues.length === 2 && h.enumValues.includes("true") && h.enumValues.includes("false");

export const labelFor = (node: Pick<CachedNode, "key" | "index">, hint?: FieldHint) =>
  hint?.title ?? (node.key !== null ? humanize(node.key) : `Item ${(node.index ?? 0) + 1}`);

/** The right control for a scalar node; used by rows, map values and list items. */
export function ScalarControl({ node, hint, labelId, descId, disabled }: { node: CachedNode; hint?: FieldHint; labelId: string; descId?: string; disabled?: boolean }) {
  switch (node.kind) {
    case "boolean":
      if (isBoolEnum(hint)) {
        return <TriStateBoolean pointer={node.pointer} parent={parentPointer(node.pointer)} keyName={lastToken(node.pointer)} value={node.text === "true" ? "on" : "off"} labelId={labelId} describedBy={descId} disabled={disabled} />;
      }
      return <BooleanSwitch node={node} labelId={labelId} disabled={disabled} />;
    case "string":
      return hint?.enumValues ? <EnumSelect node={node} hint={hint} labelId={labelId} descId={descId} disabled={disabled} /> : <TextControl node={node} hint={hint} labelId={labelId} descId={descId} disabled={disabled} />;
    case "number":
      return hint?.enumValues ? <EnumSelect node={node} hint={hint} labelId={labelId} descId={descId} disabled={disabled} /> : <NumberControl node={node} hint={hint} labelId={labelId} descId={descId} disabled={disabled} />;
    case "null":
      return <NullControl node={node} disabled={disabled} />;
    default:
      return null;
  }
}

/** Dispatcher for one node: scalar row or container section. */
export function Field({ pointer, level }: { pointer: string; level: number }) {
  const node = useNode(pointer);
  if (!node) return null;
  if (node.kind === "object" || node.kind === "array") return <ContainerField pointer={pointer} level={level} />;
  return <ScalarField node={node} />;
}

function ScalarField({ node }: { node: CachedNode }) {
  const { frozen } = useEnv();
  const hint = useHint(node.pointer);
  const labelId = idFor(node.pointer, "label");
  const descId = idFor(node.pointer, "desc");
  return (
    <FieldRow
      pointer={node.pointer}
      label={labelFor(node, hint)}
      hint={hint}
      rawKey={node.key}
      kind={node.kind}
      typeHint={node.kind === "null" ? "null" : null}
      changed={node.changed}
      removable={node.key !== null}
      labelId={labelId}
      control={<ScalarControl node={node} hint={hint} labelId={labelId} descId={descId} disabled={frozen} />}
    />
  );
}

/** Schema-known optional boolean that is not in the file: shown as Not set / On / Off, never materialised. */
export function AbsentBooleanRow({ parent, name }: { parent: string; name: string }) {
  const { frozen } = useEnv();
  const pointer = `${parent}/${name.replace(/~/g, "~0").replace(/\//g, "~1")}`;
  const hint = useHint(pointer);
  if (!isBoolEnum(hint)) return null;
  const labelId = idFor(pointer, "label");
  const descId = idFor(pointer, "desc");
  return (
    <FieldRow
      pointer={pointer}
      label={hint?.title ?? humanize(name)}
      hint={hint}
      rawKey={name}
      typeHint="not in file"
      changed={false}
      removable={false}
      labelId={labelId}
      control={<TriStateBoolean pointer={pointer} parent={parent} keyName={name} value="unset" labelId={labelId} describedBy={descId} disabled={frozen} />}
    />
  );
}

export { isBoolEnum };
