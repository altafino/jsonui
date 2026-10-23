// RFC 6901 JSON Pointer helpers. The UI keys everything by pointer.

export const ROOT = "";

export const escapeToken = (t: string) => t.replace(/~/g, "~0").replace(/\//g, "~1");
export const unescapeToken = (t: string) => t.replace(/~1/g, "/").replace(/~0/g, "~");

export function parsePointer(p: string): string[] {
  if (p === "") return [];
  return p.slice(1).split("/").map(unescapeToken);
}

export const buildPointer = (tokens: string[]) => tokens.map((t) => "/" + escapeToken(t)).join("");
export const childPointer = (parent: string, token: string | number) => parent + "/" + escapeToken(String(token));

export function parentPointer(p: string): string {
  const i = p.lastIndexOf("/");
  return i <= 0 ? "" : p.slice(0, i);
}

export const isSameOrDescendant = (p: string, ancestor: string) =>
  ancestor === "" || p === ancestor || p.startsWith(ancestor + "/");

/** Ancestors from the root down to (excluding) p. */
export function ancestorsOf(p: string): string[] {
  const toks = parsePointer(p);
  const out: string[] = [];
  for (let i = 0; i < toks.length; i++) out.push(buildPointer(toks.slice(0, i)));
  return out;
}

/** `$.logging.level`, `$.services[0].name`, `$["odd key"]`. */
export function displayPath(p: string): string {
  let out = "$";
  for (const t of parsePointer(p)) {
    if (/^\d+$/.test(t)) out += `[${t}]`;
    else if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(t)) out += `.${t}`;
    else out += `[${JSON.stringify(t)}]`;
  }
  return out;
}

/** retryCount -> "Retry count", toFile -> "To file", API_TOKEN -> "API TOKEN". */
export function humanize(key: string): string {
  if (!key) return "(empty key)";
  if (/^[A-Z0-9_]+$/.test(key)) return key.replace(/_/g, " ");
  const words = key
    .replace(/[_-]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim()
    .toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * Shared "path overlap" rule (eng R18) for the UI side of revert dependency checks:
 * two paths overlap when one is the same as, or an ancestor of, the other.
 */
export const pathsOverlap = (a: string, b: string) => isSameOrDescendant(a, b) || isSameOrDescendant(b, a);
