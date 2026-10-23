import { describe, expect, it } from "vitest";
import type { TreeNode } from "../types/generated/TreeNode";
import { checkNumberText, stepInteger } from "./numbers";
import { displayPath, humanize, pathsOverlap } from "./pointer";
import { summarizeItem } from "./summary";
import { treeToJson } from "./tree";

const n = (key: string, kind: TreeNode["kind"], text: string | null, extra: Partial<TreeNode> = {}): TreeNode => ({
  pointer: "/0/" + key, key, index: null, kind, text, childCount: null, children: null, changed: false, ...extra,
});
const item = (kids: TreeNode[]): TreeNode => ({ pointer: "/0", key: null, index: 0, kind: "object", text: null, childCount: kids.length, children: kids, changed: false });

describe("pointer helpers", () => {
  it("humanizes keys", () => {
    expect(humanize("retryCount")).toBe("Retry count");
    expect(humanize("toFile")).toBe("To file");
    expect(humanize("API_TOKEN")).toBe("API TOKEN");
    expect(humanize("max-line_length")).toBe("Max line length");
  });
  it("renders display paths from RFC 6901 pointers", () => {
    expect(displayPath("")).toBe("$");
    expect(displayPath("/logging/level")).toBe("$.logging.level");
    expect(displayPath("/services/0/name")).toBe("$.services[0].name");
    expect(displayPath("/a~1b")).toBe('$["a/b"]');
  });
  it("treats ancestors and descendants as overlapping (R18)", () => {
    expect(pathsOverlap("/a", "/a/b")).toBe(true);
    expect(pathsOverlap("/a/b", "/a")).toBe(true);
    expect(pathsOverlap("/a", "/ab")).toBe(false);
  });
});

describe("number text", () => {
  it("accepts exact JSON number lexemes and rejects the rest", () => {
    for (const ok of ["0", "-1", "1.50", "1e3", "9007199254740993", "-0.0"]) expect(checkNumberText(ok).ok).toBe(true);
    for (const bad of ["", "abc", "1.", ".5", "01", "+1", "1,5", "0x10", "NaN"]) expect(checkNumberText(bad).ok).toBe(false);
  });
  it("enforces integer and bounds", () => {
    expect(checkNumberText("1.5", { integer: true }).ok).toBe(false);
    expect(checkNumberText("11", { maximum: "10" }).ok).toBe(false);
    expect(checkNumberText("-1", { minimum: "0" }).ok).toBe(false);
    expect(checkNumberText("5", { minimum: "0", maximum: "10" }).ok).toBe(true);
  });
  it("steps integers exactly beyond 2^53", () => {
    expect(stepInteger("9007199254740993", 1)).toBe("9007199254740994");
    expect(stepInteger("0", -1)).toBe("-1");
    expect(stepInteger("1.5", 1)).toBeNull();
  });
});

describe("array item summary rule (DD17)", () => {
  it("prefers name, title, id, label, key in that order", () => {
    expect(summarizeItem(item([n("id", "string", "x1"), n("name", "string", "frontend")]), 0)).toBe("frontend · x1");
    expect(summarizeItem(item([n("key", "string", "k"), n("label", "string", "Lbl")]), 0)).toBe("Lbl · k");
  });
  it("adds up to two other scalars, booleans by key", () => {
    const s = summarizeItem(item([n("name", "string", "frontend"), n("enabled", "boolean", "true"), n("timeout", "number", "30"), n("extra", "string", "z")]), 0);
    expect(s).toBe("frontend · enabled · 30");
  });
  it("falls back to the first short string, then to Item N · k keys", () => {
    expect(summarizeItem(item([n("count", "number", "3"), n("note", "string", "short")]), 1)).toBe("short · 3");
    expect(summarizeItem(item([n("count", "number", "3"), n("on", "boolean", "true"), n("o", "object", null, { childCount: 1 }), n("a", "array", null, { childCount: 1 })]), 2)).toBe("Item 3 · 4 keys");
  });
  it("honours a preset itemSummary key", () => {
    expect(summarizeItem(item([n("name", "string", "n"), n("matcher", "string", "Bash")]), 0, "matcher")).toBe("Bash · n");
  });
});

describe("treeToJson", () => {
  it("keeps exact number lexemes and string escapes", () => {
    const t = item([n("a", "number", "1.50"), n("b", "string", 'q"'), n("c", "null", "null")]);
    expect(treeToJson(t)).toBe('{"a":1.50,"b":"q\\"","c":null}');
  });
});
