import type { DiffHunk } from "../../types/generated/DiffHunk";
import type { DiffLine } from "../../types/generated/DiffLine";
import type { MaskSpan } from "../../types/generated/MaskSpan";

export const SECRET_KEY = /(token|secret|password|passwd|api[-_]?key|auth|credential)/i;

export class MaskRegistry {
  private next = 1;
  private values = new Map<number, string>();
  add(value: string): number {
    const id = this.next++;
    this.values.set(id, value);
    return id;
  }
  get(id: number): string | undefined {
    return this.values.get(id);
  }
}

const LINE = /^(\s*"([^"]+)":\s*")((?:[^"\\]|\\.)*)(".*)$/;

/** Masks secret-like string values on one source line; returns display text + mask spans. */
export function maskLine(text: string, reg: MaskRegistry): { text: string; masks: MaskSpan[] } {
  const m = LINE.exec(text);
  if (!m || !SECRET_KEY.test(m[2]) || m[3].length === 0) return { text, masks: [] };
  const start = m[1].length;
  const id = reg.add(m[3]);
  return { text: m[1] + "•".repeat(m[3].length) + m[4], masks: [{ id, start, end: start + m[3].length }] };
}

type Op = { kind: "context" | "added" | "removed"; a?: number; b?: number; text: string };

function script(a: string[], b: string[]): Op[] {
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) out.push({ kind: "context", a: i + 1, b: j + 1, text: a[i] }), i++, j++;
    else if (dp[i + 1][j] >= dp[i][j + 1]) out.push({ kind: "removed", a: i + 1, text: a[i] }), i++;
    else out.push({ kind: "added", b: j + 1, text: b[j] }), j++;
  }
  while (i < n) out.push({ kind: "removed", a: i + 1, text: a[i] }), i++;
  while (j < m) out.push({ kind: "added", b: j + 1, text: b[j] }), j++;
  return out;
}

/** Changed hunks only, with `context` lines around them. Secret-like values are masked. */
export function textHunks(oldText: string, newText: string, reg: MaskRegistry, context = 3): DiffHunk[] {
  const ops = script(oldText.split("\n"), newText.split("\n"));
  const keep = new Array<boolean>(ops.length).fill(false);
  ops.forEach((o, idx) => {
    if (o.kind === "context") return;
    for (let k = Math.max(0, idx - context); k <= Math.min(ops.length - 1, idx + context); k++) keep[k] = true;
  });
  const hunks: DiffHunk[] = [];
  let cur: DiffLine[] | null = null;
  ops.forEach((o, idx) => {
    if (!keep[idx]) {
      cur = null;
      return;
    }
    if (!cur) {
      cur = [];
      hunks.push({ lines: cur });
    }
    const { text, masks } = maskLine(o.text, reg);
    cur.push({ kind: o.kind, oldLine: o.a ?? null, newLine: o.b ?? null, text, masks });
  });
  return hunks;
}
