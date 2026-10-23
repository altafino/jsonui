// Number text handling. Numbers stay as exact text end to end (eng R3): never via JS number for the payload.

const JSON_NUMBER = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/;
const INTEGER = /^-?(0|[1-9]\d*)$/;

export type NumberCheck = { ok: true } | { ok: false; reason: string };

export interface NumberRules {
  integer?: boolean;
  minimum?: string | null;
  maximum?: string | null;
}

/** Validates the typed text; the text itself is what gets sent. Comparison uses Number only for bounds. */
export function checkNumberText(text: string, rules: NumberRules = {}): NumberCheck {
  const t = text.trim();
  if (t === "") return { ok: false, reason: "Enter a number" };
  if (!JSON_NUMBER.test(t)) return { ok: false, reason: "Not a valid JSON number" };
  if (rules.integer && !INTEGER.test(t)) return { ok: false, reason: "Must be a whole number" };
  const n = Number(t);
  if (rules.minimum != null && n < Number(rules.minimum)) return { ok: false, reason: `Must be at least ${rules.minimum}` };
  if (rules.maximum != null && n > Number(rules.maximum)) return { ok: false, reason: `Must be at most ${rules.maximum}` };
  return { ok: true };
}

export const isIntegerText = (t: string) => INTEGER.test(t.trim());

/** Exact integer stepping with BigInt; returns null when the text is not an integer. */
export function stepInteger(text: string, delta: 1 | -1): string | null {
  if (!isIntegerText(text)) return null;
  return (BigInt(text.trim()) + BigInt(delta)).toString();
}
