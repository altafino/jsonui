// Draft semantics of the mock backend. In the real app Rust owns all of this (eng R4);
// the UI never imports this module.
import { buildPointer, childPointer, parentPointer, parsePointer, pathsOverlap } from "../../json/pointer";
import type { ChangeKind } from "../../types/generated/ChangeKind";
import type { DraftOp } from "../../types/generated/DraftOp";
import type { TreeNode } from "../../types/generated/TreeNode";
import { clone, displayText, getAt, jsonEqual, parseValueJson, type J } from "./json";

export class OpError extends Error {}

export function applyOp(root: J, op: DraftOp): J {
  const next = clone(root);
  const toks = parsePointer(op.path);
  const fail = (m: string): never => {
    throw new OpError(m);
  };
  if (op.op === "array-insert" || op.op === "array-move") {
    const arr = getAt(next, toks);
    if (!arr || arr.k !== "array") return fail("Target is not an array");
    if (op.op === "array-insert") {
      if (op.index < 0 || op.index > arr.items.length) return fail("Index out of range");
      arr.items.splice(op.index, 0, parseValueJson(op.valueJson));
    } else {
      if (op.from < 0 || op.from >= arr.items.length || op.to < 0 || op.to >= arr.items.length) return fail("Index out of range");
      const [it] = arr.items.splice(op.from, 1);
      arr.items.splice(op.to, 0, it);
    }
    return next;
  }
  if (toks.length === 0) {
    if (op.op === "set") return parseValueJson(op.valueJson);
    return fail("Cannot remove the root");
  }
  const parent = getAt(next, toks.slice(0, -1));
  const last = toks[toks.length - 1];
  if (!parent) return fail("Path does not exist");
  if (parent.k === "object") {
    const idx = parent.m.findIndex((e) => e.key === last);
    switch (op.op) {
      case "set":
        if (idx < 0) return fail("Property does not exist; use add");
        parent.m[idx].v = parseValueJson(op.valueJson);
        break;
      case "add":
        if (idx >= 0) return fail("Property already exists");
        parent.m.push({ key: last, v: parseValueJson(op.valueJson) });
        break;
      case "remove":
        if (idx < 0) return fail("Property does not exist");
        parent.m.splice(idx, 1);
        break;
      case "rename-key":
        if (idx < 0) return fail("Property does not exist");
        if (parent.m.some((e) => e.key === op.newKey)) return fail(`Key "${op.newKey}" already exists`);
        parent.m[idx].key = op.newKey;
        break;
    }
  } else if (parent.k === "array") {
    const idx = Number(last);
    if (!Number.isInteger(idx) || idx < 0 || idx >= parent.items.length) return fail("Index out of range");
    if (op.op === "set") parent.items[idx] = parseValueJson(op.valueJson);
    else if (op.op === "remove") parent.items.splice(idx, 1);
    else return fail("Operation not valid on an array item");
  } else return fail("Parent is not a container");
  return next;
}

export function replay(original: J, ops: DraftOp[]): J {
  return ops.reduce(applyOp, original);
}

// ---------------------------------------------------------------- tree

export function toTree(root: J, original: J, pointer: string, depth: number, key: string | null = null, index: number | null = null): TreeNode {
  const node = getAt(root, parsePointer(pointer));
  if (!node) throw new OpError("Pointer not found: " + pointer);
  const orig = getAt(original, parsePointer(pointer));
  const changed = !jsonEqual(node, orig);
  if (key === null && index === null && pointer !== "") {
    const toks = parsePointer(pointer);
    const par = getAt(root, toks.slice(0, -1));
    const last = toks[toks.length - 1];
    if (par?.k === "array") index = Number(last);
    else key = last;
  }
  const base = { pointer, key, index, changed };
  switch (node.k) {
    case "object":
      return {
        ...base,
        kind: "object",
        text: null,
        childCount: node.m.length,
        children: depth > 0 ? node.m.map((e) => toTree(root, original, childPointer(pointer, e.key), depth - 1, e.key, null)) : null,
      };
    case "array":
      return {
        ...base,
        kind: "array",
        text: null,
        childCount: node.items.length,
        children: depth > 0 ? node.items.map((_, i) => toTree(root, original, childPointer(pointer, i), depth - 1, null, i)) : null,
      };
    case "string":
      return { ...base, kind: "string", text: node.s, childCount: null, children: null };
    case "number":
      return { ...base, kind: "number", text: node.lex, childCount: null, children: null };
    case "boolean":
      return { ...base, kind: "boolean", text: String(node.b), childCount: null, children: null };
    case "null":
      return { ...base, kind: "null", text: "null", childCount: null, children: null };
  }
}

// ---------------------------------------------------------------- diff

export interface Entry {
  pointer: string;
  kind: ChangeKind;
  old?: J;
  next?: J;
}

/** Positional semantic diff a -> b (leaf level). Arrays compare by index. */
export function semanticDiff(a: J | undefined, b: J | undefined, pointer = "", out: Entry[] = []): Entry[] {
  if (a === undefined && b === undefined) return out;
  if (a === undefined) return out.push({ pointer, kind: "added", next: b }), out;
  if (b === undefined) return out.push({ pointer, kind: "removed", old: a }), out;
  if (a.k === "object" && b.k === "object") {
    for (const e of a.m) {
      const o = b.m.find((x) => x.key === e.key);
      semanticDiff(e.v, o?.v, childPointer(pointer, e.key), out);
    }
    for (const e of b.m) if (!a.m.some((x) => x.key === e.key)) semanticDiff(undefined, e.v, childPointer(pointer, e.key), out);
    return out;
  }
  if (a.k === "array" && b.k === "array") {
    const n = Math.max(a.items.length, b.items.length);
    for (let i = 0; i < n; i++) semanticDiff(a.items[i], b.items[i], childPointer(pointer, i), out);
    return out;
  }
  if (!jsonEqual(a, b)) out.push({ pointer, kind: "changed", old: a, next: b });
  return out;
}

/** Containers whose child list changed between a and b (keys/order/length). */
export function reshapedContainers(a: J | undefined, b: J | undefined, pointer = "", out: string[] = []): string[] {
  if (!a || !b) return out;
  if (a.k === "object" && b.k === "object") {
    if (a.m.map((e) => e.key).join("\0") !== b.m.map((e) => e.key).join("\0")) out.push(pointer);
    for (const e of a.m) {
      const o = b.m.find((x) => x.key === e.key);
      if (o) reshapedContainers(e.v, o.v, childPointer(pointer, e.key), out);
    }
  } else if (a.k === "array" && b.k === "array") {
    if (a.items.length !== b.items.length) out.push(pointer);
    for (let i = 0; i < Math.min(a.items.length, b.items.length); i++) reshapedContainers(a.items[i], b.items[i], childPointer(pointer, i), out);
  } else if (a.k !== b.k && (a.k === "object" || a.k === "array" || b.k === "object" || b.k === "array")) out.push(pointer);
  return out;
}

export function ancestorList(pointer: string): string[] {
  const t = parsePointer(pointer);
  const out: string[] = [];
  for (let i = 0; i <= t.length; i++) out.push(buildPointer(t.slice(0, i)));
  return out;
}

export function opTouches(op: DraftOp, pointer: string): boolean {
  return pathsOverlap(op.path, pointer);
}

/** Last op index touching `pointer`, or null. */
export function lastOpFor(ops: DraftOp[], pointer: string): number | null {
  for (let i = ops.length - 1; i >= 0; i--) if (opTouches(ops[i], pointer)) return i;
  return null;
}

/** A change is revertible unless a later op touches the same path region (DD15, R18). */
export function isRevertible(ops: DraftOp[], opIndex: number | null): boolean {
  if (opIndex === null) return false;
  const p = ops[opIndex].path;
  return !ops.slice(opIndex + 1).some((o) => pathsOverlap(o.path, p));
}

export const entryText = (j: J | undefined) => displayText(j);
export { parentPointer };
