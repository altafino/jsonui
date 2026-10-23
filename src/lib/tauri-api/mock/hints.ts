import { parsePointer } from "../../json/pointer";
import type { FieldHint } from "../../types/generated/FieldHint";
import type { PresetInfo } from "../../types/generated/PresetInfo";
import { getAt, type J } from "./json";

const blank = (pointer: string): FieldHint => ({
  pointer, valueType: null, required: false, title: null, description: null, enumValues: null, defaultJson: null, format: null, minimum: null, maximum: null,
  minLength: null, maxLength: null, pattern: null, secret: false, addableProperties: [], openMap: false, itemSummary: null, sensitive: false,
});

type Table = Record<string, Partial<FieldHint>>;

const FLAGS: Table = {
  "": { addableProperties: ["verboseLogging", "region", "homepage"] },
  "/application": { title: "Application", minLength: 1 },
  "/enabled": { title: "Enabled" },
  "/retryCount": { minimum: "0", maximum: "10", format: "integer", description: "How many times a failed call is retried." },
  "/timeoutSeconds": { minimum: "0" },
  "/optionalValue": { description: "Left empty on purpose." },
  "/verboseLogging": { title: "Verbose logging", enumValues: ["true", "false"], defaultJson: "false", description: "Log every request." },
  "/region": { title: "Region", enumValues: ['"eu"', '"us"'] },
  "/homepage": { title: "Homepage", format: "uri" },
  "/logging": { addableProperties: ["format"] },
  "/logging/level": { enumValues: ['"debug"', '"info"', '"warn"', '"error"'] },
  "/services": { title: "Services", itemSummary: "name" },
};

const CLAUDE: Table = {
  "": { addableProperties: ["theme", "includeCoAuthoredBy", "cleanupPeriodDays"] },
  "/model": { title: "Default model", description: "Model used by default." },
  "/permissions": { title: "Permissions", sensitive: true, addableProperties: ["additionalDirectories"] },
  "/permissions/defaultMode": {
    title: "Default permission mode",
    enumValues: ['"default"', '"acceptEdits"', '"plan"', '"bypassPermissions"'],
    sensitive: true,
  },
  "/permissions/allow": { title: "Allow rules", sensitive: true },
  "/permissions/deny": { title: "Deny rules" },
  "/env": { title: "Environment variables", openMap: true },
  "/hooks": { title: "Hooks" },
  "/hooks/PreToolUse": { itemSummary: "matcher" },
  "/includeCoAuthoredBy": { title: "Include co-authored-by", enumValues: ["true", "false"], defaultJson: "true" },
  "/theme": { title: "Theme", enumValues: ['"dark"', '"light"'] },
  "/cleanupPeriodDays": { title: "Cleanup period (days)", minimum: "1", format: "integer" },
};

const TABLES: Record<string, Table> = { "feature-flags": FLAGS, "claude-code": CLAUDE };

export function hintFor(presetId: string | null, pointer: string, doc: J | null): FieldHint {
  const h = { ...blank(pointer), ...(presetId ? TABLES[presetId]?.[pointer] ?? {} : {}) };
  // Addable properties disappear once present in the file.
  if (doc && h.addableProperties.length) {
    const node = getAt(doc, parsePointer(pointer));
    if (node?.k === "object") h.addableProperties = h.addableProperties.filter((n) => !node.m.some((e) => e.key === n));
  }
  const last = parsePointer(pointer).slice(-1)[0] ?? "";
  if (/(token|secret|password|api[-_]?key)/i.test(last)) h.secret = true;
  return h;
}

export function presetInfoFor(presetId: string | null): PresetInfo | null {
  if (presetId !== "claude-code") return null;
  return {
    presetId, displayName: "Claude Code", version: "1",
    groups: [
      { id: "general", title: "General", pointers: ["/model", "/includeCoAuthoredBy", "/theme"] },
      { id: "permissions", title: "Permissions", pointers: ["/permissions"] },
      { id: "env", title: "Environment variables", pointers: ["/env"] },
      { id: "hooks", title: "Hooks", pointers: ["/hooks"] },
    ],
  };
}
