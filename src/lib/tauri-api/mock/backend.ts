// In-memory simulation of the Rust backend for dev in a browser, Playwright and Vitest.
// Mirrors docs/contract.md. Never imported by the UI directly; selected in tauri-api/index.ts.
import { childPointer, parentPointer, parsePointer } from "../../json/pointer";
import type { BackupEntry } from "../../types/generated/BackupEntry";
import type { ChangeSummary } from "../../types/generated/ChangeSummary";
import type { ConflictInfo } from "../../types/generated/ConflictInfo";
import type { CoreError } from "../../types/generated/CoreError";
import type { Diagnostic } from "../../types/generated/Diagnostic";
import type { DiscoveredCandidate } from "../../types/generated/DiscoveredCandidate";
import type { DocumentSnapshot } from "../../types/generated/DocumentSnapshot";
import type { DraftOp } from "../../types/generated/DraftOp";
import type { EditResult } from "../../types/generated/EditResult";
import type { FileStatus } from "../../types/generated/FileStatus";
import type { ManagedDocument } from "../../types/generated/ManagedDocument";
import type { Notice } from "../../types/generated/Notice";
import type { Preferences } from "../../types/generated/Preferences";
import type { SaveResult } from "../../types/generated/SaveResult";
import type { SemanticChange } from "../../types/generated/SemanticChange";
import type { TreeNode } from "../../types/generated/TreeNode";
import type { Api, EventMap, Events, MockControls } from "../types";
import { FIXTURES, claudeHooksSettings } from "./fixtures";
import { hintFor, presetInfoFor } from "./hints";
import { clone, displayText, getAt, parseJson, serialize, type J } from "./json";
import {
  OpError, ancestorList, applyOp, isRevertible, lastOpFor, reshapedContainers, replay, semanticDiff, toTree, type Entry,
} from "./model";
import { MaskRegistry, SECRET_KEY, maskLine, textHunks } from "./textdiff";

interface Disk {
  text: string;
  exists: boolean;
  readable: boolean;
  writable: boolean;
  isSymlink: boolean;
  hardlinks: number;
}

interface Open {
  original: J | null;
  originalText: string;
  fingerprint: string;
  ops: DraftOp[];
  redo: DraftOp[];
  draft: J | null;
  revision: number;
  masks: MaskRegistry;
  conflict: boolean;
}

export interface MockOptions {
  /** Comma flags: seeded, recovered, leftover, missing, readonly, symlink. */
  flags?: string[];
  /** Milliseconds a save takes (to exercise the 150 ms spinner rule). */
  saveDelayMs?: number;
}

const DEFAULT_PREFS: Preferences = { theme: "system", density: "comfortable", fontScale: 1, backupRetention: 20, hideSecrets: true, sidebarWidth: 320 };

const fingerprintOf = (text: string) => {
  let h = 5381;
  for (let i = 0; i < text.length; i++) h = ((h << 5) + h + text.charCodeAt(i)) | 0;
  return `fp-${(h >>> 0).toString(16)}-${text.length}`;
};

export function coreError(code: CoreError["code"], message: string, remediation: string | null = null, extra: Partial<CoreError> = {}): CoreError {
  return { code, message, path: null, line: null, column: null, remediation, ...extra };
}

export function createMockBackend(options: MockOptions = {}) {
  const flags = new Set(options.flags ?? []);
  let saveDelay = options.saveDelayMs ?? 0;
  let failNextSave: CoreError["code"] | null = null;
  let seq = 0;
  const id = (p: string) => `${p}-${++seq}`;

  let registry: ManagedDocument[] = [];
  const disks = new Map<string, Disk>();
  const opens = new Map<string, Open>();
  const backups = new Map<string, { entry: BackupEntry; text: string }[]>();
  let prefs: Preferences = { ...DEFAULT_PREFS };
  const listeners = new Map<string, Set<(p: never) => void>>();

  const emit = <K extends keyof EventMap>(event: K, payload: EventMap[K]) => listeners.get(event)?.forEach((h) => (h as (p: EventMap[K]) => void)(payload));

  const events: Events = {
    listen: async (event, handler) => {
      const set = listeners.get(event) ?? new Set();
      set.add(handler as (p: never) => void);
      listeners.set(event, set);
      return () => set.delete(handler as (p: never) => void);
    },
  };

  // ------------------------------------------------------------ registry

  function register(partial: Partial<ManagedDocument> & { displayName: string; absolutePath: string }, text: string, disk: Partial<Disk> = {}): ManagedDocument {
    const doc: ManagedDocument = {
      id: id("f"), kind: partial.absolutePath.endsWith(".jsonc") ? "jsonc" : "json", entryKind: "file", presetId: null, localSchemaPath: null,
      collectionId: null, discoveredScope: null, readOnly: false, linkWriteConfirmed: false, hardlinkWriteConfirmed: false, lastOpenedAt: null,
      ...partial,
    };
    registry.push(doc);
    disks.set(doc.id, { text, exists: true, readable: true, writable: !doc.readOnly, isSymlink: false, hardlinks: 1, ...disk });
    return doc;
  }

  function seed() {
    const base = "~/projects/demo";
    register({ displayName: "User settings", absolutePath: "~/.claude/settings.json", presetId: "claude-code", collectionId: "claude-code", discoveredScope: "user" }, FIXTURES.claudeSettings);
    register({ displayName: "my-app · Shared", absolutePath: "~/projects/my-app/.claude/settings.json", presetId: "claude-code", collectionId: "claude-code", discoveredScope: "project-shared" }, claudeHooksSettings);
    register({ displayName: "my-app · Local", absolutePath: "~/projects/my-app/.claude/settings.local.json", presetId: "claude-code", collectionId: "claude-code", discoveredScope: "project-local" }, '{\n  "permissions": {\n    "allow": []\n  }\n}\n');
    register({ displayName: "feature-flags.json", absolutePath: `${base}/feature-flags.json`, presetId: "feature-flags" }, FIXTURES.featureFlags);
    register({ displayName: "app-config.json", absolutePath: `${base}/app-config.json` }, FIXTURES.invalidMissingComma);
    register({ displayName: "duplicate-keys.json", absolutePath: `${base}/duplicate-keys.json` }, FIXTURES.duplicateKeys);
    register({ displayName: "legacy-comments.json", absolutePath: `${base}/legacy-comments.json` }, FIXTURES.invalidComment);
    register({ displayName: "broken-secrets.json", absolutePath: `${base}/broken-secrets.json` }, '{\n  "env": {\n    "API_TOKEN": "sk-test-0000000000000000"\n    "name": "demo"\n  }\n}\n');
    register({ displayName: "secrets.json", absolutePath: `${base}/secrets.json` }, FIXTURES.secret);
    register({ displayName: "deep.json", absolutePath: `${base}/deep.json` }, FIXTURES.deep);
    register({ displayName: "numbers.json", absolutePath: `${base}/numbers.json` }, FIXTURES.numbers);
    register({ displayName: "settings.jsonc", absolutePath: `${base}/settings.jsonc` }, FIXTURES.commented);
    register({ displayName: "empty.json", absolutePath: `${base}/empty.json` }, FIXTURES.emptyObject);
    if (flags.has("readonly")) register({ displayName: "Managed settings", absolutePath: "/etc/claude-code/managed-settings.json", presetId: "claude-code", collectionId: "claude-code", discoveredScope: "managed", readOnly: true }, FIXTURES.claudeSettings, { writable: false });
    if (flags.has("symlink")) register({ displayName: "linked-config.json", absolutePath: `${base}/linked-config.json` }, '{\n  "enabled": true\n}\n', { isSymlink: true });
    if (flags.has("missing")) {
      const d = register({ displayName: "gone.json", absolutePath: `${base}/gone.json` }, "{}");
      disks.get(d.id)!.exists = false;
    }
  }
  if (flags.has("seeded")) seed();

  const entry = (fileId: string) => {
    const e = registry.find((d) => d.id === fileId);
    if (!e) throw coreError("NOT_FOUND", "This file is not registered in ConfigDeck.");
    return e;
  };
  const diskOf = (fileId: string) => disks.get(fileId)!;

  // ------------------------------------------------------------ documents

  function loadOpen(fileId: string): Open {
    const e = entry(fileId);
    const d = diskOf(fileId);
    if (!d.exists) throw coreError("NOT_FOUND", `This file no longer exists at ${e.absolutePath}`);
    const parsed = parseJson(d.text, e.kind === "jsonc");
    const original = parsed.ok ? parsed.value : null;
    return { original, originalText: d.text, fingerprint: fingerprintOf(d.text), ops: [], redo: [], draft: original ? clone(original) : null, revision: 1, masks: new MaskRegistry(), conflict: false };
  }

  const openOf = (fileId: string): Open => {
    const o = opens.get(fileId);
    if (!o) throw coreError("INVALID_OPERATION", "Document is not open.");
    return o;
  };

  function sourceViewFor(fileId: string, o: Open) {
    const e = entry(fileId);
    const parsed = parseJson(o.originalText, e.kind === "jsonc");
    const lines = o.originalText.split("\n");
    const masks: { id: number; start: number; end: number }[] = [];
    const text: string[] = [];
    let offset = 0;
    for (const line of lines) {
      const m = maskLine(line, o.masks);
      for (const s of m.masks) masks.push({ id: s.id, start: offset + s.start, end: offset + s.end });
      text.push(m.text);
      offset += line.length + 1;
    }
    const hl = parsed.ok ? [] : parsed.extraLines ?? [parsed.line];
    return { text: text.join("\n"), masks, highlightLines: hl, caret: parsed.ok ? null : { line: parsed.line, column: parsed.column } };
  }

  function baseDiagnostics(fileId: string, o: Open): Diagnostic[] {
    const e = entry(fileId);
    const parsed = parseJson(o.originalText, e.kind === "jsonc");
    if (parsed.ok) return [];
    return [{ severity: "error", source: parsed.code === "DUPLICATE_KEYS" ? "duplicateKey" : "syntax", code: parsed.code, message: parsed.message, pointer: null, line: parsed.line, column: parsed.column }];
  }

  /** Schema-ish checks from the mock's local "schema" (preset warnings vs user-schema errors). */
  function schemaDiagnostics(fileId: string, o: Open): Diagnostic[] {
    const e = entry(fileId);
    const out: Diagnostic[] = [];
    if (!o.draft) return out;
    const at = (p: string) => getAt(o.draft!, parsePointer(p));
    if (e.presetId === "feature-flags") {
      const app = at("/application");
      if (app?.k === "string" && app.s.trim() === "") out.push({ severity: "error", source: "userSchema", code: "MIN_LENGTH", message: "Application must not be empty.", pointer: "/application", line: null, column: null });
      const rc = at("/retryCount");
      if (rc?.k === "number" && Number(rc.lex) > 10) out.push({ severity: "warning", source: "presetSchema", code: "MAXIMUM", message: "Retry count above 10 is unusual (maximum 10).", pointer: "/retryCount", line: null, column: null });
      const lvl = at("/logging/level");
      if (lvl?.k === "string" && !["debug", "info", "warn", "error"].includes(lvl.s)) out.push({ severity: "warning", source: "presetSchema", code: "ENUM", message: `Level "${lvl.s}" is not one of debug, info, warn, error.`, pointer: "/logging/level", line: null, column: null });
    }
    if (e.presetId === "claude-code") {
      const m = at("/permissions/defaultMode");
      if (m?.k === "string" && m.s === "bypassPermissions") out.push({ severity: "warning", source: "presetSchema", code: "SENSITIVE", message: "bypassPermissions skips all permission prompts.", pointer: "/permissions/defaultMode", line: null, column: null });
    }
    return out;
  }

  function snapshot(fileId: string): DocumentSnapshot {
    const e = entry(fileId);
    const o = openOf(fileId);
    const d = diskOf(fileId);
    return {
      fileId, kind: e.kind, fingerprint: o.fingerprint, sizeBytes: o.originalText.length, hasBom: false, lineEnding: "lf",
      editable: !!o.draft, readOnly: e.readOnly || !d.writable,
      root: o.draft ? toTree(o.draft, o.original!, "", 1) : null,
      sourceView: o.draft ? null : sourceViewFor(fileId, o),
      diagnostics: [...baseDiagnostics(fileId, o), ...schemaDiagnostics(fileId, o)],
      revision: o.revision, dirtyCount: dirtyCount(o), canUndo: o.ops.length > 0, canRedo: o.redo.length > 0,
    };
  }

  const dirtyCount = (o: Open) => (o.draft && o.original ? semanticDiff(o.original, o.draft).length : 0);

  function changeEntries(o: Open): SemanticChange[] {
    return entriesToChanges(semanticDiff(o.original!, o.draft!), o);
  }

  function entriesToChanges(entries: Entry[], o: Open | null, ops: DraftOp[] = o?.ops ?? []): SemanticChange[] {
    const out: SemanticChange[] = [];
    const used = new Set<Entry>();
    // Coalesce rename-key ops: removed(old) + added(new) with equal values.
    ops.forEach((op) => {
      if (op.op !== "rename-key") return;
      const par = parentPointer(op.path);
      const oldK = parsePointer(op.path).slice(-1)[0];
      const rem = entries.find((x) => x.kind === "removed" && x.pointer === op.path);
      const add = entries.find((x) => x.kind === "added" && x.pointer === childPointer(par, op.newKey));
      if (rem && add && serialize(rem.old!) === serialize(add.next!)) {
        used.add(rem);
        used.add(add);
        const idx = ops.indexOf(op);
        out.push({ pointer: add.pointer, kind: "renamed", oldText: oldK, newText: op.newKey, masked: false, opIndex: idx, revertible: isRevertible(ops, idx) });
      }
    });
    for (const x of entries) {
      if (used.has(x)) continue;
      const key = parsePointer(x.pointer).slice(-1)[0] ?? "";
      const secret = SECRET_KEY.test(key) && ((x.old?.k ?? x.next?.k) === "string");
      const idx = o ? lastOpFor(ops, x.pointer) : null;
      out.push({
        pointer: x.pointer, kind: x.kind, oldText: secret ? "••••" : displayText(x.old), newText: secret ? "••••" : displayText(x.next), masked: secret,
        opIndex: idx, revertible: o ? isRevertible(ops, idx) : false,
      });
    }
    return out;
  }

  function editResult(o: Open, before: J, after: J, forceReshaped: string[] = []): EditResult {
    const diff = semanticDiff(before, after);
    const reshaped = [...new Set([...reshapedContainers(before, after), ...forceReshaped])];
    const removed = diff.filter((x) => x.kind === "removed").map((x) => x.pointer);
    const pts = new Set<string>();
    for (const x of diff) if (x.kind !== "removed") ancestorList(x.pointer).forEach((p) => pts.add(p));
    for (const r of [...reshaped, ...removed.map(parentPointer)]) ancestorList(r).forEach((p) => pts.add(p));
    // The changed flag of every touched node can flip when undoing back to the original.
    for (const x of semanticDiff(o.original!, before)) ancestorList(x.pointer).forEach((p) => getAt(after, parsePointer(p)) && pts.add(p));
    const changed = [...pts].filter((p) => getAt(after, parsePointer(p))).map((p) => toTree(after, o.original!, p, 1));
    scheduleDiagnostics(o);
    return { revision: o.revision, changed, removed, reshaped, dirtyCount: dirtyCount(o), canUndo: o.ops.length > 0, canRedo: o.redo.length > 0, diagnostics: [] };
  }

  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  function scheduleDiagnostics(o: Open) {
    const fileId = [...opens.entries()].find(([, v]) => v === o)?.[0];
    if (!fileId) return;
    clearTimeout(timers.get(fileId));
    timers.set(fileId, setTimeout(() => emit("draft-diagnostics", { fileId, revision: o.revision, diagnostics: schemaDiagnostics(fileId, o) }), 150));
  }

  function guardEditable(fileId: string, o: Open) {
    const e = entry(fileId);
    if (!o.draft) throw coreError("NOT_EDITABLE", "This file cannot be edited until its syntax errors are fixed.");
    if (e.readOnly || !diskOf(fileId).writable) throw coreError("READ_ONLY", "This file is read-only.", "Make the file writable outside ConfigDeck, then reload.");
  }

  function mutate(o: Open, ops: DraftOp[], redo: DraftOp[], forceReshaped: string[] = []): EditResult {
    const before = o.draft!;
    let after: J;
    try {
      after = replay(o.original!, ops);
    } catch (e) {
      if (e instanceof OpError) throw coreError("INVALID_OPERATION", e.message);
      throw e;
    }
    o.ops = ops;
    o.redo = redo;
    o.draft = after;
    o.revision++;
    return editResult(o, before, after, forceReshaped);
  }

  const forcedReshape = (op: DraftOp): string[] => (op.op === "array-move" ? [op.path] : []);

  // ------------------------------------------------------------ api

  const wait = (ms = 0) => new Promise<void>((r) => setTimeout(r, ms));

  function conflictFor(fileId: string): ConflictInfo {
    const e = entry(fileId);
    const o = openOf(fileId);
    const d = diskOf(fileId);
    const parsed = parseJson(d.text, e.kind === "jsonc");
    const draftChanges = changeEntries(o);
    if (!parsed.ok) {
      return {
        diskFingerprint: fingerprintOf(d.text), diskChanges: [], draftChanges, overlapping: [], canReapply: false,
        diskParseError: { severity: "error", source: "syntax", code: parsed.code, message: parsed.message, pointer: null, line: parsed.line, column: parsed.column },
      };
    }
    const diskEntries = semanticDiff(o.original!, parsed.value);
    const diskChanges = entriesToChanges(diskEntries, null);
    // DD16/R16: overlap when the op path, an ancestor, a descendant, or its parent array changed on disk.
    const overlapping = draftChanges
      .map((c) => c.pointer)
      .filter((p) =>
        diskEntries.some((x) => {
          const a = parsePointer(p);
          const b = parsePointer(x.pointer);
          const n = Math.min(a.length, b.length);
          if (a.slice(0, n).join("/") === b.slice(0, n).join("/")) return true;
          const par = getAt(o.original!, a.slice(0, -1));
          if (par?.k === "array") {
            const pp = a.slice(0, -1);
            return b.length >= pp.length && b.slice(0, pp.length).join("/") === pp.join("/");
          }
          return false;
        }),
      );
    return { diskFingerprint: fingerprintOf(d.text), diskChanges, draftChanges, overlapping, canReapply: overlapping.length === 0, diskParseError: null };
  }

  function summaryFor(fileId: string, baseText: string, baseModel: J | null, target: J | null, o: Open, withOps: boolean): ChangeSummary {
    const e = entry(fileId);
    const targetText = target ? serialize(target) + "\n" : "";
    const baseNorm = baseModel ? serialize(baseModel) + "\n" : baseText;
    const diags = withOps ? schemaDiagnostics(fileId, o) : [];
    void e;
    return {
      changes: baseModel && target ? entriesToChanges(semanticDiff(baseModel, target), withOps ? o : null) : [],
      hunks: textHunks(baseNorm, targetText, o.masks),
      warnings: diags.filter((x) => x.severity === "warning"),
      errors: diags.filter((x) => x.severity === "error"),
    };
  }

  function writeDisk(fileId: string, text: string, makeBackup: boolean, changeCount: number | null): SaveResult {
    const d = diskOf(fileId);
    let backupId: string | null = null;
    if (makeBackup) {
      backupId = id("b");
      const list = backups.get(fileId) ?? [];
      list.unshift({
        entry: { id: backupId, fileId, createdAt: new Date().toISOString(), sizeBytes: d.text.length, changeCount, location: `~/.local/share/configdeck/backups/${fileId}/${backupId}.json` },
        text: d.text,
      });
      backups.set(fileId, list.slice(0, prefs.backupRetention));
    }
    d.text = text;
    return { fingerprint: fingerprintOf(text), backupId, savedAt: new Date().toISOString() };
  }

  const api: Api = {
    async listManagedFiles() {
      return registry.map((d) => ({ ...d }));
    },
    async getFileStatuses() {
      return registry.map<FileStatus>((d) => {
        const k = diskOf(d.id);
        return { fileId: d.id, exists: k.exists, readable: k.readable, writable: k.writable, isSymlink: k.isSymlink, linkTarget: k.isSymlink ? "/srv/shared/config.json" : null, hardlinkCount: k.hardlinks, sizeBytes: k.exists ? k.text.length : null, editable: k.exists ? parseJson(k.text, d.kind === "jsonc").ok : null };
      });
    },
    async getStartupNotices() {
      const out: Notice[] = [];
      if (flags.has("recovered")) out.push({ code: "REGISTRY_RECOVERED", message: "ConfigDeck's file list could not be read and was reset. A copy of the damaged file was kept.", path: "~/.local/share/configdeck/registry.json.bad" });
      return out;
    },
    async pickAndAddManagedFile(displayName) {
      await wait();
      const n = registry.length + 1;
      return register({ displayName: displayName ?? `picked-${n}.json`, absolutePath: `~/projects/demo/picked-${n}.json` }, '{\n  "name": "picked",\n  "enabled": true\n}\n');
    },
    async pickAndCreateManagedFile() {
      await wait();
      const n = registry.length + 1;
      return register({ displayName: `new-${n}.json`, absolutePath: `~/projects/demo/new-${n}.json` }, "{}\n");
    },
    async createSampleFile() {
      const ex = registry.find((d) => d.entryKind === "sample");
      if (ex) return { ...ex };
      return register({ displayName: "feature-flags.json", absolutePath: "~/.local/share/configdeck/samples/feature-flags.json", entryKind: "sample", presetId: "feature-flags", collectionId: "sample" }, FIXTURES.featureFlags);
    },
    async removeManagedFile(fileId) {
      entry(fileId);
      registry = registry.filter((d) => d.id !== fileId);
      opens.delete(fileId);
      disks.delete(fileId);
    },
    async updateManagedFileMetadata(fileId, patch) {
      const e = entry(fileId);
      if (patch.displayName !== null) e.displayName = patch.displayName;
      if (patch.collectionId !== null) e.collectionId = patch.collectionId;
      return { ...e };
    },
    async relocateManagedFile(fileId) {
      const e = entry(fileId);
      const d = diskOf(fileId);
      d.exists = true;
      opens.delete(fileId);
      return { ...e };
    },
    async createMissingFile(fileId) {
      const d = diskOf(fileId);
      if (d.exists) throw coreError("IO", "The file already exists");
      d.exists = true;
      d.text = "{}\n";
      opens.delete(fileId);
    },
    async confirmLinkWrite(fileId) {
      const e = entry(fileId);
      e.linkWriteConfirmed = true;
      e.hardlinkWriteConfirmed = true;
      return { ...e };
    },
    async listDiscoverableClaudeFiles() {
      const has = (p: string) => registry.some((d) => d.absolutePath === p);
      const list: DiscoveredCandidate[] = [
        { candidateId: "c-user", path: "~/.claude/settings.json", scope: "user", exists: flags.has("seeded") || flags.has("discover"), readOnly: false, alreadyRegistered: has("~/.claude/settings.json"), note: null },
        { candidateId: "c-managed", path: "/etc/claude-code/managed-settings.json", scope: "managed", exists: flags.has("discover"), readOnly: true, alreadyRegistered: has("/etc/claude-code/managed-settings.json"), note: "Managed by your organization; read-only." },
      ];
      return list;
    },
    async pickProjectDirectory() {
      await wait();
      return [
        { candidateId: "c-shared", path: "~/projects/my-app/.claude/settings.json", scope: "project-shared", exists: true, readOnly: false, alreadyRegistered: false, note: "Committed to version control and shared with your team." },
        { candidateId: "c-local", path: "~/projects/my-app/.claude/settings.local.json", scope: "project-local", exists: true, readOnly: false, alreadyRegistered: false, note: "Personal to you; keep it out of version control." },
      ];
    },
    async addDiscoveredFile(candidateId) {
      const map: Record<string, { name: string; path: string; scope: ManagedDocument["discoveredScope"]; text: string; ro?: boolean }> = {
        "c-user": { name: "User settings", path: "~/.claude/settings.json", scope: "user", text: FIXTURES.claudeSettings },
        "c-managed": { name: "Managed settings", path: "/etc/claude-code/managed-settings.json", scope: "managed", text: FIXTURES.claudeSettings, ro: true },
        "c-shared": { name: "my-app · Shared", path: "~/projects/my-app/.claude/settings.json", scope: "project-shared", text: claudeHooksSettings },
        "c-local": { name: "my-app · Local", path: "~/projects/my-app/.claude/settings.local.json", scope: "project-local", text: "{}\n" },
      };
      const c = map[candidateId];
      if (!c) throw coreError("NOT_FOUND", "Unknown candidate.");
      return register({ displayName: c.name, absolutePath: c.path, presetId: "claude-code", collectionId: "claude-code", discoveredScope: c.scope, readOnly: !!c.ro }, c.text, { writable: !c.ro });
    },
    async openDocument(fileId) {
      entry(fileId).lastOpenedAt = new Date().toISOString();
      if (!opens.has(fileId)) opens.set(fileId, loadOpen(fileId));
      return snapshot(fileId);
    },
    async reloadDocument(fileId) {
      opens.set(fileId, loadOpen(fileId));
      return snapshot(fileId);
    },
    async getSubtree(fileId, pointer, depth) {
      const o = openOf(fileId);
      if (!o.draft) throw coreError("NOT_EDITABLE", "This file has no editable tree.");
      return toTree(o.draft, o.original!, pointer, depth);
    },
    async getFieldHints(fileId, pointers) {
      const o = openOf(fileId);
      const e = entry(fileId);
      return pointers.map((p) => hintFor(e.presetId, p, o.draft));
    },
    async getPresetInfo(fileId) {
      return presetInfoFor(entry(fileId).presetId);
    },
    async applyDraftOperation(fileId, op) {
      const o = openOf(fileId);
      guardEditable(fileId, o);
      if (o.conflict) throw coreError("EXTERNAL_CHANGE_CONFLICT", "This file changed outside ConfigDeck. Resolve the conflict first.");
      return mutate(o, [...o.ops, op], [], forcedReshape(op));
    },
    async undoDraftOperation(fileId) {
      const o = openOf(fileId);
      if (!o.ops.length) throw coreError("INVALID_OPERATION", "Nothing to undo.");
      const last = o.ops[o.ops.length - 1];
      return mutate(o, o.ops.slice(0, -1), [...o.redo, last], forcedReshape(last));
    },
    async redoDraftOperation(fileId) {
      const o = openOf(fileId);
      if (!o.redo.length) throw coreError("INVALID_OPERATION", "Nothing to redo.");
      const next = o.redo[o.redo.length - 1];
      return mutate(o, [...o.ops, next], o.redo.slice(0, -1), forcedReshape(next));
    },
    async revertChange(fileId, opIndex) {
      const o = openOf(fileId);
      if (!isRevertible(o.ops, opIndex)) throw coreError("INVALID_OPERATION", "A later change depends on this one.", `Revert later changes under ${o.ops[opIndex]?.path ?? ""} first.`);
      const gone = o.ops[opIndex];
      return mutate(o, o.ops.filter((_, i) => i !== opIndex), [], [parentPointer(gone.path), gone.path]);
    },
    async discardDraft(fileId) {
      opens.set(fileId, { ...loadOpen(fileId) });
      return snapshot(fileId);
    },
    async previewDraft(fileId) {
      const o = openOf(fileId);
      return summaryFor(fileId, o.originalText, o.original, o.draft, o, true);
    },
    async saveDraft(fileId, expectedFingerprint, acceptWarnings) {
      const e = entry(fileId);
      const o = openOf(fileId);
      const d = diskOf(fileId);
      await wait(saveDelay);
      if (e.readOnly || !d.writable) throw coreError("READ_ONLY", "This file is read-only.");
      if (failNextSave) {
        const code = failNextSave;
        failNextSave = null;
        throw coreError(code, code === "BACKUP_FAILED" ? "Backup folder is not writable." : "The file could not be written.", code === "BACKUP_FAILED" ? "Choose another backup folder in Settings." : "Check the file's permissions and try again.");
      }
      if (fingerprintOf(d.text) !== expectedFingerprint || fingerprintOf(d.text) !== o.fingerprint) {
        o.conflict = true;
        throw coreError("EXTERNAL_CHANGE_CONFLICT", "This file changed outside ConfigDeck.", "Compare the versions, then reapply, reload or save a copy.");
      }
      if (d.isSymlink && !e.linkWriteConfirmed) throw coreError("SYMLINK_REQUIRES_CONFIRMATION", "This file is a symbolic link.", "Confirm to write through the link to its target.");
      if (d.hardlinks > 1 && !e.hardlinkWriteConfirmed) throw coreError("HARDLINK_REQUIRES_CONFIRMATION", "This file has other hard links.", "Saving replaces the file and detaches those links.");
      const diags = schemaDiagnostics(fileId, o);
      if (diags.some((x) => x.severity === "error")) throw coreError("SCHEMA_VALIDATION_FAILED", "The draft has errors.", "Fix the errors, then save.");
      const warns = diags.filter((x) => x.severity === "warning");
      if (warns.length && !acceptWarnings) throw coreError("PRESET_WARNINGS_REQUIRE_CONFIRMATION", `${warns.length} warning${warns.length === 1 ? "" : "s"} from the preset.`);
      const count = dirtyCount(o);
      const text = serialize(o.draft!) + "\n";
      const res = writeDisk(fileId, text, true, count);
      o.original = clone(o.draft!);
      o.originalText = text;
      o.fingerprint = res.fingerprint;
      o.ops = [];
      o.redo = [];
      o.revision++;
      o.conflict = false;
      return res;
    },
    async getConflict(fileId) {
      return conflictFor(fileId);
    },
    async reapplyDraft(fileId) {
      const e = entry(fileId);
      const o = openOf(fileId);
      const c = conflictFor(fileId);
      if (!c.canReapply) throw coreError("EXTERNAL_CHANGE_CONFLICT", "Overlapping changes cannot be reapplied.");
      const d = diskOf(fileId);
      const parsed = parseJson(d.text, e.kind === "jsonc");
      if (!parsed.ok) throw coreError("INVALID_JSON", parsed.message);
      const ops = o.ops;
      const next = replay(parsed.value, ops);
      o.original = parsed.value;
      o.originalText = d.text;
      o.fingerprint = fingerprintOf(d.text);
      o.draft = next;
      o.revision++;
      o.conflict = false;
      scheduleDiagnostics(o);
      return { revision: o.revision, changed: [toTree(next, parsed.value, "", 1)], removed: [], reshaped: [""], dirtyCount: dirtyCount(o), canUndo: ops.length > 0, canRedo: false, diagnostics: [] };
    },
    async saveDraftAsCopy(fileId) {
      const e = entry(fileId);
      const o = openOf(fileId);
      await wait();
      const base = e.displayName.replace(/\.jsonc?$/, "");
      return register({ displayName: `${base} (copy).json`, absolutePath: e.absolutePath.replace(/\.jsonc?$/, "") + ".copy.json", presetId: e.presetId, collectionId: e.collectionId }, serialize(o.draft!) + "\n");
    },
    async listBackups(fileId) {
      entry(fileId);
      return (backups.get(fileId) ?? []).map((b) => ({ ...b.entry }));
    },
    async previewRestore(fileId, backupId) {
      const e = entry(fileId);
      const o = openOf(fileId);
      const b = backups.get(fileId)?.find((x) => x.entry.id === backupId);
      if (!b) throw coreError("NOT_FOUND", "Backup not found.");
      const cur = parseJson(diskOf(fileId).text, e.kind === "jsonc");
      const target = parseJson(b.text, e.kind === "jsonc");
      return summaryFor(fileId, diskOf(fileId).text, cur.ok ? cur.value : null, target.ok ? target.value : null, o, false);
    },
    async restoreBackup(fileId, backupId, expectedFingerprint) {
      const e = entry(fileId);
      const d = diskOf(fileId);
      if (e.readOnly) throw coreError("READ_ONLY", "This file is read-only.");
      if (fingerprintOf(d.text) !== expectedFingerprint) throw coreError("EXTERNAL_CHANGE_CONFLICT", "This file changed outside ConfigDeck.");
      const b = backups.get(fileId)?.find((x) => x.entry.id === backupId);
      if (!b) throw coreError("NOT_FOUND", "Backup not found.");
      const res = writeDisk(fileId, b.text, true, null);
      opens.set(fileId, loadOpen(fileId));
      return res;
    },
    async pickSchema(fileId) {
      return { ...entry(fileId), localSchemaPath: "~/schemas/custom.schema.json" };
    },
    async clearSchema(fileId) {
      const e = entry(fileId);
      e.localSchemaPath = null;
      return { ...e };
    },
    async revealManagedFile() {},
    async openInExternalEditor() {},
    async revealMaskedValue(fileId, maskId) {
      const v = openOf(fileId).masks.get(maskId);
      if (v === undefined) throw coreError("NOT_FOUND", "Masked value not found.");
      return v;
    },
    async listLeftoverTempFiles(fileId) {
      entry(fileId);
      return flags.has("leftover") && registry[0]?.id === fileId ? [".settings.json.configdeck-tmp"] : [];
    },
    async removeLeftoverTempFiles(fileId) {
      entry(fileId);
      flags.delete("leftover");
      return 1;
    },
    async getPreferences() {
      return { ...prefs };
    },
    async setPreferences(p) {
      prefs = { ...p };
    },
  };

  const controls: MockControls = {
    simulateExternalChange(fileId, mode = "non-overlapping") {
      const e = entry(fileId);
      const d = diskOf(fileId);
      const o = opens.get(fileId);
      if (mode === "invalid") d.text = '{\n  "broken": \n';
      else {
        const parsed = parseJson(d.text, e.kind === "jsonc");
        if (!parsed.ok) return;
        const model = clone(parsed.value);
        const touched = (o?.ops ?? []).map((x) => x.path);
        const leaves = semanticDiff({ k: "object", m: [] }, model).filter((x) => x.next && x.next.k !== "object" && x.next.k !== "array").map((x) => x.pointer);
        const overlaps = (p: string) => touched.some((t) => t === p || t.startsWith(p + "/") || p.startsWith(t + "/"));
        const target = mode === "overlapping" ? (touched[0] ?? leaves[0]) : leaves.filter((p) => !overlaps(p) && !/\/\d+$/.test(p)).slice(-1)[0] ?? leaves[0];
        const cur = getAt(model, parsePointer(target));
        const nv: J = cur?.k === "boolean" ? { k: "boolean", b: !cur.b } : cur?.k === "number" ? { k: "number", lex: "42" } : { k: "string", s: "changed-on-disk" };
        d.text = serialize(applyOp(model, { op: "set", path: target, valueJson: serialize(nv) })) + "\n";
      }
      const fp = fingerprintOf(d.text);
      if (o && dirtyCount(o) > 0) o.conflict = true;
      emit("managed-file-external-change", { fileId, fingerprint: fp, hasUnsavedChanges: !!o && dirtyCount(o) > 0 });
    },
    simulateMissing(fileId) {
      diskOf(fileId).exists = false;
      emit("managed-file-missing", { fileId });
    },
    simulateSaveFailure(code) {
      failNextSave = code;
    },
    setSaveDelay(ms) {
      saveDelay = ms;
    },
    idOf(displayName) {
      return registry.find((d) => d.displayName === displayName)?.id;
    },
    reset() {
      registry = [];
      disks.clear();
      opens.clear();
      backups.clear();
      prefs = { ...DEFAULT_PREFS };
    },
  };

  return { api, events, controls };
}

export type MockBackend = ReturnType<typeof createMockBackend>;
export type { TreeNode };
