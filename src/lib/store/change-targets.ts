// Which rows the save bar and the review dialog can scroll to.
import { parentPointer } from "../json/pointer";
import type { SemanticChange } from "../types/generated/SemanticChange";
import { isContainer, type NodeCache } from "./cache";

/**
 * Pointers of pending edits, in review order.
 * A removed field scrolls to its parent row. Changed leaves stand in until the
 * preview lists at least as many targets.
 */
function changedLeaves(nodes: NodeCache): string[] {
  return Object.values(nodes)
    .filter((n) => n.pointer !== "" && n.changed && !isContainer(n))
    .map((n) => n.pointer)
    .sort((a, b) => a.localeCompare(b));
}

export function changePointers(changes: SemanticChange[], nodes: NodeCache): string[] {
  const leaves = changedLeaves(nodes);
  if (changes.length === 0) return leaves;
  const out: string[] = [];
  for (const c of changes) {
    const p = nodes[c.pointer] ? c.pointer : parentPointer(c.pointer);
    if (!p || !nodes[p] || out.includes(p)) continue;
    out.push(p);
  }
  // A preview from the previous edit is shorter than the leaves already on screen.
  if (leaves.length > out.length) return leaves;
  return out;
}
