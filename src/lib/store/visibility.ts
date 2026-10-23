import { displayPath, humanize } from "../json/pointer";
import type { FieldHint } from "../types/generated/FieldHint";
import type { NodeCache } from "./cache";

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
    const h = hints[ptr];
    const hay = [h?.title ?? (n.key !== null ? humanize(n.key) : `Item ${(n.index ?? 0) + 1}`), n.key ?? "", displayPath(ptr), h?.description ?? ""];
    return hay.some((s) => s.toLowerCase().includes(q));
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
