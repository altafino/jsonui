import type { DocState } from "../../lib/store/doc-store";

/** The container the toolbar's Add setting acts on: the drill root when it is an object. Null otherwise. */
export function drillTarget(doc: Pick<DocState, "nodes" | "drillRoot">): string | null {
  const n = doc.nodes[doc.drillRoot] ?? doc.nodes[""];
  return n && n.kind === "object" && (n.childCount ?? 0) > 0 ? (doc.nodes[doc.drillRoot] ? doc.drillRoot : "") : null;
}
