import type { TreeNode } from "../types/generated/TreeNode";

const PREFERRED_KEYS = ["name", "title", "id", "label", "key"];
const SHORT_STRING = 40;

const scalarText = (n: TreeNode) => (n.kind === "string" ? (n.text ?? "") : (n.text ?? ""));
const isScalar = (n: TreeNode) => n.kind !== "object" && n.kind !== "array";

/**
 * Array-of-objects item summary (DD17).
 * First present of name/title/id/label/key (or the preset `itemSummary` key), else the first short string
 * field, plus up to two other scalars ("frontend · enabled"). Fallback: "Item 3 · 4 keys".
 */
export function summarizeItem(item: TreeNode, index: number, itemSummaryKey?: string | null): string {
  const kids = item.children ?? [];
  const byKey = (k: string) => kids.find((c) => c.key === k && isScalar(c));
  const lead =
    (itemSummaryKey ? byKey(itemSummaryKey) : undefined) ??
    PREFERRED_KEYS.map(byKey).find(Boolean) ??
    kids.find((c) => c.kind === "string" && (c.text ?? "").length > 0 && (c.text ?? "").length <= SHORT_STRING);
  if (!lead) {
    const n = item.childCount ?? kids.length;
    return `Item ${index + 1} · ${n} ${n === 1 ? "key" : "keys"}`;
  }
  const extras = kids
    .filter((c) => c !== lead && isScalar(c) && c.kind !== "null")
    .slice(0, 2)
    .map((c) => (c.kind === "boolean" ? (c.text === "true" ? c.key! : `${c.key} off`) : scalarText(c)));
  const leadText = scalarText(lead);
  return [leadText, ...extras].join(" · ");
}
