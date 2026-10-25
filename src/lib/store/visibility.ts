import { displayPath, humanize } from "../json/pointer";
import type { FieldHint } from "../types/generated/FieldHint";
import type { CachedNode, NodeCache } from "./cache";

export interface Filter {
  query: string;
  changedOnly: boolean;
}

/**
 * Pointers to show for field search / Changed only (spec §4): matches by friendly label, raw key, path
 * and schema description, plus their ancestors. Returns null when no filter is active.
 */
export function computeVisible(nodes: NodeCache, root: string, filter: Filter, hints: Record<string, FieldHint>): Set<string> | null {
  const q = filter.query.trim().toLowerCase();
  if (!q && !filter.changedOnly) return null;
  const out = new Set<string>();

  const matches = (ptr: string) => {
    const n = nodes[ptr];
    return !!n && fieldMatches(n, q, hints);
  };

  const visit = (ptr: string, inherited: boolean): boolean => {
    const n = nodes[ptr];
    if (!n) return false;
    const self = !q || matches(ptr);
    const cover = inherited || (!!q && self);
    const passesChanged = !filter.changedOnly || n.changed;
    let ok: boolean;
    if (n.kind === "object" || n.kind === "array") {
      const kids = (n.childPointers ?? []).map((c) => visit(c, cover));
      ok = kids.some(Boolean) || (cover && !n.childPointers?.length && passesChanged);
    } else {
      ok = (cover || self) && passesChanged;
    }
    if (ok) out.add(ptr);
    return ok;
  };

  const r = nodes[root];
  for (const c of r?.childPointers ?? []) visit(c, false);
  return out;
}

function fieldMatches(n: CachedNode, q: string, hints: Record<string, FieldHint>): boolean {
  const h = hints[n.pointer];
  const hay = [h?.title ?? (n.key !== null ? humanize(n.key) : `Item ${(n.index ?? 0) + 1}`), n.key ?? "", displayPath(n.pointer), h?.description ?? ""];
  return hay.some((s) => s.toLowerCase().includes(q));
}

/** Self-matches of a form filter against every loaded field except the root. Null when the query is empty. */
export function countQueryMatches(nodes: NodeCache, query: string, hints: Record<string, FieldHint>): { matches: number; total: number } | null {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  let matches = 0;
  let total = 0;
  for (const n of Object.values(nodes)) {
    if (n.pointer === "") continue;
    total += 1;
    if (fieldMatches(n, q, hints)) matches += 1;
  }
  return { matches, total };
}
