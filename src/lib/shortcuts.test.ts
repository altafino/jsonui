import { describe, expect, it } from "vitest";
import { matchShortcut } from "./shortcuts";

const ev = (key: string, o: Partial<{ ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean; target: EventTarget | null }> = {}) => ({
  key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, target: null, ...o,
});
const input = () => document.createElement("input");

describe("shortcut map (DD14)", () => {
  it("maps Ctrl combos on Linux/Windows", () => {
    expect(matchShortcut(ev("k", { ctrlKey: true }), false)).toBe("command-search");
    expect(matchShortcut(ev("f", { ctrlKey: true }), false)).toBe("field-search");
    expect(matchShortcut(ev("s", { ctrlKey: true }), false)).toBe("save");
    expect(matchShortcut(ev("b", { ctrlKey: true }), false)).toBe("toggle-sidebar");
    expect(matchShortcut(ev("ArrowLeft", { altKey: true }), false)).toBe("back");
    expect(matchShortcut(ev("F6"), false)).toBe("cycle-landmark");
    expect(matchShortcut(ev("F6", { shiftKey: true }), false)).toBe("cycle-landmark-back");
  });
  it("uses Cmd on macOS and ignores Ctrl there", () => {
    expect(matchShortcut(ev("s", { metaKey: true }), true)).toBe("save");
    expect(matchShortcut(ev("s", { ctrlKey: true }), true)).toBeNull();
  });
  it("undo and redo only when focus is not in a text field", () => {
    expect(matchShortcut(ev("z", { ctrlKey: true }), false)).toBe("undo");
    expect(matchShortcut(ev("z", { ctrlKey: true, shiftKey: true }), false)).toBe("redo");
    expect(matchShortcut(ev("z", { ctrlKey: true, target: input() }), false)).toBeNull();
    const box = document.createElement("input");
    box.type = "checkbox";
    expect(matchShortcut(ev("z", { ctrlKey: true, target: box }), false)).toBe("undo");
    expect(matchShortcut(ev("z", { ctrlKey: true, target: document.createElement("textarea") }), false)).toBeNull();
  });
  it("leaves unrelated keys alone", () => {
    expect(matchShortcut(ev("a"), false)).toBeNull();
    expect(matchShortcut(ev("s", { ctrlKey: true, shiftKey: true }), false)).toBeNull();
  });
});
