import type { TreeNode } from "../types/generated/TreeNode";

/** Serialises a fully loaded TreeNode back to JSON text; numbers keep their exact lexeme. */
export function treeToJson(n: TreeNode): string {
  switch (n.kind) {
    case "object":
      return "{" + (n.children ?? []).map((c) => `${JSON.stringify(c.key ?? "")}:${treeToJson(c)}`).join(",") + "}";
    case "array":
      return "[" + (n.children ?? []).map(treeToJson).join(",") + "]";
    case "string":
      return JSON.stringify(n.text ?? "");
    default:
      return n.text ?? "null";
  }
}
