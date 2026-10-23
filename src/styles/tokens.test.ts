/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync("src/styles/tokens.css", "utf8");

function block(selector: RegExp): Record<string, string> {
  const m = selector.exec(css);
  if (!m) throw new Error("block not found");
  return Object.fromEntries([...m[1].matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/g)].map((x) => [x[1], x[2]]));
}
const light = block(/:root\s*\{([^}]*)\}/);
const dark = { ...light, ...block(/:root\[data-theme="dark"\]\s*\{([^}]*)\}/) };

const lum = (hex: string) => {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
};
const ratio = (a: string, b: string) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

describe("DD9/DD10/DD13 tokens", () => {
  it("matches the approved hex values", () => {
    expect(dark).toMatchObject({ bg: "#0b1020", sidebar: "#10172a", border: "#1e293b", text: "#e2e8f0", muted: "#94a3b8", accent: "#6366f1", "accent-text": "#818cf8" });
    expect(light).toMatchObject({ bg: "#ffffff", sidebar: "#f8fafc", border: "#e2e8f0", text: "#0f172a", muted: "#475569", accent: "#4f46e5" });
    expect(dark.primary).toBe("#4f46e5");
    expect(dark["primary-hover"]).toBe("#4338ca");
    expect(dark["control-border"]).toBe("#64748b");
    expect(light["control-border"]).toBe("#64748b");
  });

  for (const [name, t] of [["light", light], ["dark", dark]] as const) {
    it(`${name}: every text pair is at least 4.5:1`, () => {
      const pairs: [string, string, string][] = [
        ["text", t.text, t.bg], ["text on sidebar", t.text, t.sidebar], ["text on row hover", t.text, t["row-hover"]],
        ["muted", t.muted, t.bg], ["muted on sidebar", t.muted, t.sidebar], ["muted on row hover", t.muted, t["row-hover"]],
        ["accent text", t["accent-text"], t.bg], ["error", t.error, t.bg], ["warning", t.warning, t.bg], ["success", t.success, t.bg],
        ["error on sidebar", t.error, t.sidebar], ["warning on sidebar", t.warning, t.sidebar], ["success on sidebar", t.success, t.sidebar],
        ["on-primary", t["on-primary"], t.primary], ["on-primary hover", t["on-primary"], t["primary-hover"]],
      ];
      for (const [label, fg, bg] of pairs) expect(ratio(fg, bg), label).toBeGreaterThanOrEqual(4.5);
    });
    it(`${name}: control border is at least 3:1 against the background`, () => {
      expect(ratio(t["control-border"], t.bg)).toBeGreaterThanOrEqual(3);
      expect(ratio(t["control-border"], t.sidebar)).toBeGreaterThanOrEqual(3);
    });
  }
});
