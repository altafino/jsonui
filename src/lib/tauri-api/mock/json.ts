// Tiny exact-number JSON model for the mock backend only (numbers keep their lexeme).
export type J =
  | { k: "object"; m: { key: string; v: J }[] }
  | { k: "array"; items: J[] }
  | { k: "string"; s: string }
  | { k: "number"; lex: string }
  | { k: "boolean"; b: boolean }
  | { k: "null" };

export type ParseResult =
  | { ok: true; value: J; duplicates: { key: string; lines: number[] }[] }
  | { ok: false; code: "INVALID_JSON" | "DUPLICATE_KEYS"; message: string; line: number; column: number; extraLines?: number[] };

class Fail extends Error {
  constructor(public pos: number, msg: string) {
    super(msg);
  }
}

export function parseJson(text: string, allowComments: boolean): ParseResult {
  let i = 0;
  const dups: { key: string; lines: number[] }[] = [];
  const lineOf = (pos: number) => text.slice(0, pos).split("\n").length;
  const colOf = (pos: number) => pos - text.lastIndexOf("\n", pos - 1);

  const ws = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text.startsWith("//", i) || text.startsWith("/*", i)) {
        if (!allowComments) throw new Fail(i, "Comments are not allowed in .json files");
        if (text.startsWith("//", i)) while (i < text.length && text[i] !== "\n") i++;
        else {
          const e = text.indexOf("*/", i + 2);
          i = e < 0 ? text.length : e + 2;
        }
      } else return;
    }
  };
  const str = (): string => {
    const start = i;
    i++;
    while (i < text.length && text[i] !== '"') {
      if (text[i] === "\\") i++;
      if (text[i] === "\n") throw new Fail(i, "Unterminated string");
      i++;
    }
    if (i >= text.length) throw new Fail(start, "Unterminated string");
    i++;
    try {
      return JSON.parse(text.slice(start, i)) as string;
    } catch {
      throw new Fail(start, "Invalid string escape");
    }
  };
  const value = (): J => {
    ws();
    const c = text[i];
    if (c === "{") {
      i++;
      const m: { key: string; v: J }[] = [];
      const seen = new Map<string, number[]>();
      ws();
      if (text[i] === "}") {
        i++;
        return { k: "object", m };
      }
      for (;;) {
        ws();
        if (text[i] !== '"') throw new Fail(i, "Expected a property name in quotes here");
        const kpos = i;
        const key = str();
        ws();
        if (text[i] !== ":") throw new Fail(i, "Expected a colon after the property name");
        i++;
        const v = value();
        m.push({ key, v });
        const l = seen.get(key) ?? [];
        l.push(lineOf(kpos));
        seen.set(key, l);
        ws();
        if (text[i] === ",") {
          i++;
          ws();
          if (allowComments && text[i] === "}") {
            i++;
            break;
          }
          continue;
        }
        if (text[i] === "}") {
          i++;
          break;
        }
        throw new Fail(i, "Expected a comma or a closing brace here");
      }
      for (const [key, lines] of seen) if (lines.length > 1) dups.push({ key, lines });
      return { k: "object", m };
    }
    if (c === "[") {
      i++;
      const items: J[] = [];
      ws();
      if (text[i] === "]") {
        i++;
        return { k: "array", items };
      }
      for (;;) {
        items.push(value());
        ws();
        if (text[i] === ",") {
          i++;
          ws();
          if (allowComments && text[i] === "]") {
            i++;
            break;
          }
          continue;
        }
        if (text[i] === "]") {
          i++;
          break;
        }
        throw new Fail(i, "Expected a comma or a closing bracket here");
      }
      return { k: "array", items };
    }
    if (c === '"') return { k: "string", s: str() };
    const lit = /^(true|false|null)/.exec(text.slice(i, i + 5));
    if (lit) {
      i += lit[1].length;
      return lit[1] === "null" ? { k: "null" } : { k: "boolean", b: lit[1] === "true" };
    }
    const num = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?/.exec(text.slice(i));
    if (num) {
      i += num[0].length;
      return { k: "number", lex: num[0] };
    }
    throw new Fail(i, i >= text.length ? "Unexpected end of file" : "Unexpected character");
  };

  try {
    const v = value();
    ws();
    if (i < text.length) throw new Fail(i, "Unexpected content after the end of the value");
    if (dups.length) {
      const d = dups[0];
      return {
        ok: false,
        code: "DUPLICATE_KEYS",
        message: `Key \`${d.key}\` appears twice: lines ${d.lines.join(" and ")}`,
        line: d.lines[0],
        column: 1,
        extraLines: dups.flatMap((x) => x.lines),
      };
    }
    return { ok: true, value: v, duplicates: dups };
  } catch (e) {
    if (e instanceof Fail) return { ok: false, code: "INVALID_JSON", message: e.message, line: lineOf(e.pos), column: colOf(e.pos) };
    throw e;
  }
}

const isScalar = (j: J) => j.k !== "object" && j.k !== "array";

export function serialize(j: J, indent = 0): string {
  const pad = "  ".repeat(indent);
  switch (j.k) {
    case "string":
      return JSON.stringify(j.s);
    case "number":
      return j.lex;
    case "boolean":
      return String(j.b);
    case "null":
      return "null";
    case "array": {
      if (j.items.length === 0) return "[]";
      if (j.items.every(isScalar)) {
        const inline = "[" + j.items.map((x) => serialize(x)).join(", ") + "]";
        if (inline.length <= 80) return inline;
      }
      return "[\n" + j.items.map((x) => pad + "  " + serialize(x, indent + 1)).join(",\n") + "\n" + pad + "]";
    }
    case "object": {
      if (j.m.length === 0) return "{}";
      return "{\n" + j.m.map((e) => `${pad}  ${JSON.stringify(e.key)}: ${serialize(e.v, indent + 1)}`).join(",\n") + "\n" + pad + "}";
    }
  }
}

export const clone = (j: J): J => JSON.parse(JSON.stringify(j)) as J;

export function getAt(root: J, tokens: string[]): J | undefined {
  let cur: J | undefined = root;
  for (const t of tokens) {
    if (!cur) return undefined;
    if (cur.k === "object") cur = cur.m.find((e) => e.key === t)?.v;
    else if (cur.k === "array" && /^\d+$/.test(t)) cur = cur.items[Number(t)];
    else return undefined;
  }
  return cur;
}

/** Parses strict JSON value text sent by the UI (valueJson). */
export function parseValueJson(text: string): J {
  const r = parseJson(text, false);
  if (!r.ok) throw new Error("valueJson is not valid JSON: " + r.message);
  return r.value;
}

/** Short display text for review/diff lists. */
export function displayText(j: J | undefined): string | null {
  if (!j) return null;
  switch (j.k) {
    case "string":
      return JSON.stringify(j.s);
    case "object":
      return `{${j.m.length} ${j.m.length === 1 ? "key" : "keys"}}`;
    case "array":
      return `[${j.items.length} ${j.items.length === 1 ? "item" : "items"}]`;
    default:
      return serialize(j);
  }
}

export const jsonEqual = (a: J | undefined, b: J | undefined) =>
  a === undefined || b === undefined ? a === b : serialize(a) === serialize(b);
