import { describe, expect, it } from "vitest";
import type { EditResult } from "../types/generated/EditResult";
import type { TreeNode } from "../types/generated/TreeNode";
import { applyEditResult, putTree, type NodeCache } from "./cache";
import { saveGate, saveReducer, type SaveState } from "./save-machine";
import { computeVisible } from "./visibility";

const leaf = (pointer: string, kind: TreeNode["kind"], text: string, changed = false): TreeNode => ({
  pointer, key: pointer.split("/").pop() ?? null, index: null, kind, text, childCount: null, children: null, changed,
});
const obj = (pointer: string, children: TreeNode[] | null, changed = false, count = children?.length ?? 0): TreeNode => ({
  pointer, key: pointer.split("/").pop() || null, index: null, kind: "object", text: null, childCount: count, children, changed,
});
const res = (p: Partial<EditResult>): EditResult => ({ revision: 2, changed: [], removed: [], reshaped: [], dirtyCount: 1, canUndo: true, canRedo: false, diagnostics: [], ...p });

function seed(): NodeCache {
  const c: NodeCache = {};
  putTree(c, obj("", [leaf("/debug", "boolean", "false"), obj("/logging", [leaf("/logging/level", "string", "info"), obj("/logging/deep", [leaf("/logging/deep/x", "number", "1")])])]));
  return c;
}

describe("node cache from EditResult (R13)", () => {
  it("replaces a changed leaf and its ancestors without dropping unrelated loaded subtrees", () => {
    const next = applyEditResult(seed(), res({
      changed: [
        leaf("/debug", "boolean", "true", true),
        obj("", [leaf("/debug", "boolean", "true", true), obj("/logging", null, false, 2)], true),
      ],
    }));
    expect(next["/debug"].text).toBe("true");
    expect(next["/debug"].changed).toBe(true);
    expect(next[""].changed).toBe(true);
    // /logging was untouched, so its loaded children survive
    expect(next["/logging"].childPointers).toEqual(["/logging/level", "/logging/deep"]);
    expect(next["/logging/deep/x"]).toBeDefined();
  });

  it("drops removed pointers with their descendants", () => {
    const next = applyEditResult(seed(), res({ removed: ["/logging/deep"], changed: [obj("/logging", [leaf("/logging/level", "string", "info")], true)], reshaped: ["/logging"] }));
    expect(next["/logging/deep"]).toBeUndefined();
    expect(next["/logging/deep/x"]).toBeUndefined();
    expect(next["/logging"].childPointers).toEqual(["/logging/level"]);
  });

  it("invalidates descendants of reshaped containers", () => {
    const next = applyEditResult(seed(), res({ reshaped: ["/logging"], changed: [obj("/logging", [leaf("/logging/level", "string", "info"), obj("/logging/deep", null, false, 1)], true)] }));
    expect(next["/logging/deep"].childPointers).toBeNull();
    expect(next["/logging/deep/x"]).toBeUndefined();
  });

  it("clears everything when the root is reshaped", () => {
    const next = applyEditResult(seed(), res({ reshaped: [""], changed: [obj("", [leaf("/debug", "boolean", "false")])] }));
    expect(Object.keys(next).sort()).toEqual(["", "/debug"]);
  });
});

describe("save state machine (DD6)", () => {
  const run = (events: Parameters<typeof saveReducer>[1][], from: SaveState = { phase: "idle" }) => events.reduce(saveReducer, from);

  it("shows the spinner only after SPIN", () => {
    expect(run([{ type: "START" }])).toEqual({ phase: "saving", spinner: false });
    expect(run([{ type: "START" }, { type: "SPIN" }])).toEqual({ phase: "saving", spinner: true });
  });
  it("ignores a late SPIN once the save finished", () => {
    const s = run([{ type: "START" }, { type: "SUCCESS", at: "t", backupId: "b1" }, { type: "SPIN" }]);
    expect(s.phase).toBe("saved");
  });
  it("saved -> settled after the timer, backup flag follows the backup id", () => {
    const s = run([{ type: "START" }, { type: "SUCCESS", at: "t", backupId: "b1" }]);
    expect(s).toMatchObject({ phase: "saved", backup: true, backupId: "b1" });
    expect(saveReducer(s, { type: "SETTLE" })).toEqual({ phase: "settled" });
  });
  it("keeps a failure until dismissed, but a new edit clears success", () => {
    const err = { code: "ATOMIC_WRITE_FAILED", message: "m", path: null, line: null, column: null, remediation: null } as const;
    const failed = run([{ type: "START" }, { type: "FAIL", error: err }]);
    expect(saveReducer(failed, { type: "EDIT" }).phase).toBe("error");
    expect(saveReducer(failed, { type: "DISMISS" }).phase).toBe("idle");
    expect(saveReducer({ phase: "settled" }, { type: "EDIT" }).phase).toBe("idle");
  });
  it("gates save with a visible reason", () => {
    const base = { dirty: 2, errors: 0, readOnly: false, conflict: false, editable: true, saving: false };
    expect(saveGate(base)).toEqual({ enabled: true, reason: null });
    expect(saveGate({ ...base, dirty: 0 }).reason).toBe("No unsaved changes");
    expect(saveGate({ ...base, errors: 1 }).reason).toBe("Fix 1 error to save");
    expect(saveGate({ ...base, errors: 3 }).reason).toBe("Fix 3 errors to save");
    expect(saveGate({ ...base, readOnly: true }).enabled).toBe(false);
    expect(saveGate({ ...base, conflict: true }).reason).toMatch(/outside change/);
  });
});

describe("field filter", () => {
  it("matches labels, keys and paths and keeps ancestors; Changed only filters by flag", () => {
    const c = seed();
    const v = computeVisible(c, "", { query: "level", changedOnly: false }, {})!;
    expect([...v].sort()).toEqual(["/logging", "/logging/level"]);
    expect(computeVisible(c, "", { query: "", changedOnly: true }, {})!.size).toBe(0);
    expect(computeVisible(c, "", { query: "", changedOnly: false }, {})).toBeNull();
    // a matching container shows all of its children
    expect(computeVisible(c, "", { query: "logging", changedOnly: false }, {})!.has("/logging/deep/x")).toBe(true);
  });
});
