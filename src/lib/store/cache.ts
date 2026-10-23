// Pointer-keyed node cache (eng R13). Pure functions so they are easy to test.
import { isSameOrDescendant } from "../json/pointer";
import type { EditResult } from "../types/generated/EditResult";
import type { TreeNode } from "../types/generated/TreeNode";

export type CachedNode = Omit<TreeNode, "children"> & {
  /** null = children not loaded yet; [] = loaded and empty. */
  childPointers: string[] | null;
};

export type NodeCache = Record<string, CachedNode>;

const shallow = (n: TreeNode, childPointers: string[] | null): CachedNode => {
  const { children: _children, ...rest } = n;
  void _children;
  return { ...rest, childPointers };
};

/** Inserts a node and every loaded descendant it carries. */
export function putTree(cache: NodeCache, node: TreeNode): void {
  cache[node.pointer] = shallow(node, node.children ? node.children.map((c) => c.pointer) : null);
  node.children?.forEach((c) => putTree(cache, c));
}

export function dropDescendants(cache: NodeCache, pointer: string): void {
  for (const p of Object.keys(cache)) if (p !== pointer && isSameOrDescendant(p, pointer)) delete cache[p];
}

/**
 * Applies an apply/undo/redo/revert/reapply response: replace the changed nodes,
 * drop removed pointers and the descendants of reshaped containers.
 * Subtrees of untouched children are kept; nothing outside the response is refetched.
 */
export function applyEditResult(prev: NodeCache, res: Pick<EditResult, "changed" | "removed" | "reshaped">): NodeCache {
  const cache: NodeCache = { ...prev };
  const changedSet = new Set(res.changed.map((n) => n.pointer));
  const reshaped = new Set(res.reshaped);

  for (const p of res.removed) {
    delete cache[p];
    dropDescendants(cache, p);
  }
  for (const p of res.reshaped) {
    if (p === "") for (const k of Object.keys(cache)) delete cache[k];
    else dropDescendants(cache, p);
  }

  for (const n of res.changed) {
    const existing = cache[n.pointer];
    cache[n.pointer] = shallow(n, n.children ? n.children.map((c) => c.pointer) : existing && existing.childCount === n.childCount && !reshaped.has(n.pointer) ? existing.childPointers : null);
    if (!n.children) continue;
    for (const c of n.children) {
      if (changedSet.has(c.pointer)) continue;
      const old = cache[c.pointer];
      const keep = !!old && old.kind === c.kind && old.childCount === c.childCount && !reshaped.has(c.pointer);
      cache[c.pointer] = shallow(c, keep ? old.childPointers : null);
      if (!keep) dropDescendants(cache, c.pointer);
    }
  }
  return cache;
}

export function isContainer(n: Pick<CachedNode, "kind"> | undefined): boolean {
  return n?.kind === "object" || n?.kind === "array";
}
